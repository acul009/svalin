use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_rpc::rpc::{
    command::{
        dispatcher::CommandDispatcher,
        handler::{PermissionPrecursor, TakeableCommandHandler},
    },
    session::Session,
};
use svalin_sysctl::pty::{PtyProcess, TerminalSize};
use tokio::{select, sync::mpsc};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::debug;

use crate::permissions::Permission;

#[derive(Debug, Serialize, Deserialize)]
pub enum TerminalPacket {
    Close,
    Input(Vec<u8>),
    Resize(TerminalSize),
}

impl From<TerminalInput> for TerminalPacket {
    fn from(data: TerminalInput) -> Self {
        match data {
            TerminalInput::Input(input) => TerminalPacket::Input(input),
            TerminalInput::Resize(size) => TerminalPacket::Resize(size),
        }
    }
}

pub struct RemoteTerminalHandler;

impl From<&PermissionPrecursor<RemoteTerminalHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<RemoteTerminalHandler>) -> Self {
        Permission::RootOnlyPlaceholder
    }
}

#[async_trait]
impl TakeableCommandHandler for RemoteTerminalHandler {
    type Request = ();

    fn key() -> String {
        "remote-terminal".into()
    }

    async fn handle(
        &self,
        session: &mut Option<Session>,
        _: Self::Request,
        cancel: CancellationToken,
    ) -> Result<()> {
        if let Some(mut session) = session.take() {
            let tasks = TaskTracker::new();
            let size: TerminalSize = session.read_object().await?;
            let (pty, mut pty_reader) = PtyProcess::shell(size).await?;

            let (mut read, mut write, _) = session.destructure();

            tasks.spawn(async move {
                loop {
                    select! {
                        output = pty_reader.recv() => {
                            if let Err(err) = write.write_object(&output).await {
                                tracing::error!("{err}");
                                return;
                            }

                            if output.is_none() {
                                let _ = write.shutdown().await;
                                return;
                            }
                        }
                    }
                }
            });

            loop {
                select! {
                    _ = cancel.cancelled() => {
                        pty.close();
                        break;
                    }
                    packet = read.read_object() => {
                        let packet = packet?;

                        debug!("got terminal packet: {packet:?}");
                        match packet {
                            TerminalPacket::Close => {
                                pty.close();
                                cancel.cancel();
                                return Ok(());
                            }
                            TerminalPacket::Input(input) => {
                                if let Err(err) = pty.write(input).await {
                                    tracing::error!("{err}");
                                    return Err(err);
                                }
                            }
                            TerminalPacket::Resize(size) => pty.resize(size).unwrap(),
                        };
                    }
                }
            }

            tasks.close();
            tasks.wait().await;

            Ok(())
        } else {
            Err(anyhow!("tried executing commandhandler with None"))
        }
    }
}

pub struct RemoteTerminalDispatcher {
    pub input: mpsc::Receiver<TerminalInput>,
    pub output: mpsc::Sender<Result<Vec<u8>, ()>>,
    pub cancel: CancellationToken,
}

impl CommandDispatcher for RemoteTerminalDispatcher {
    type Output = ();
    type Error = anyhow::Error;
    type Request = ();

    fn key() -> String {
        RemoteTerminalHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &()
    }

    async fn dispatch(mut self, session: &mut Session) -> Result<Self::Output, Self::Error> {
        debug!("starting remote terminal");

        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    break;
                },
                input = self.input.recv() => {
                    match input {
                        Some(input) => {
                            session.write_object(&TerminalPacket::from(input)).await?;
                        },
                        None => {
                            break;
                        }
                    }
                },
                output = session.read_object::<Option<Vec<u8>>>() => {
                    match output {
                        Ok(Some(chunk)) => {
                            if let Err(err) = self.output.send(Ok(chunk)).await {
                                tracing::error!("{err}");
                            }
                        },
                        Ok(None) => {
                            break;
                        },
                        Err(err) => {
                            tracing::error!("{err}");
                            break;
                        }
                    }
                }
            }
        }

        session.write_object(&TerminalPacket::Close).await?;

        Ok(())
    }
}
