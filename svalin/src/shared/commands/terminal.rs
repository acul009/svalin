use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_rpc::rpc::{
    command::{
        dispatcher::TakeableCommandDispatcher,
        handler::{PermissionPrecursor, TakeableCommandHandler},
    },
    session::Session,
};
use svalin_sysctl::pty::{PtyProcess, TerminalSize};
use tokio::{select, sync::mpsc, task::JoinSet};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::debug;

use crate::permissions::Permission;

#[derive(Debug, Serialize, Deserialize)]
pub enum TerminalPacket {
    Close,
    Input(Vec<u8>),
    Resize(TerminalSize),
}

pub struct RemoteTerminalHandler;

impl From<&PermissionPrecursor<(), RemoteTerminalHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<(), RemoteTerminalHandler>) -> Self {
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
            let (pty, mut pty_reader) = PtyProcess::shell(size)?;

            let (mut read, mut write, _) = session.destructure();

            tasks.spawn(async move {
                loop {
                    select! {
                        output = pty_reader.recv() => {

                            match output {
                                Some(output) => {
                                    if let Err(err) = write.write_object(&output).await {
                                        tracing::error!("{err}");
                                        return;
                                    } else {
                                        if let Ok(debug_string) = String::from_utf8(output) {
                                            tracing::debug!("wrote to terminal: {debug_string}");
                                        }
                                    }
                                }
                                None => {
                                    let _ = write.shutdown().await;
                                    return;
                                }
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

pub struct RemoteTerminal {
    input: mpsc::Sender<TerminalPacket>,
    output: tokio::sync::Mutex<mpsc::Receiver<Vec<u8>>>,
    joinset: JoinSet<()>,
}

impl RemoteTerminal {
    pub async fn write(&self, content: String) {
        debug!("writing to remote terminal: {content}");
        if let Err(err) = self.input.send(TerminalPacket::Input(content.into())).await {
            tracing::error!("{err}");
        }
    }

    pub async fn resize(&self, size: TerminalSize) {
        if let Err(err) = self.input.send(TerminalPacket::Resize(size)).await {
            tracing::error!("{err}");
        }
    }

    pub async fn read(&self) -> Result<Option<String>> {
        match self.output.lock().await.recv().await {
            Some(chunk) => Ok(Some(String::from_utf8(chunk)?)),
            None => Ok(None),
        }
    }
}

impl Drop for RemoteTerminal {
    fn drop(&mut self) {
        match self.input.try_send(TerminalPacket::Close) {
            Ok(_) => (),
            Err(_) => self.joinset.abort_all(),
        }
    }
}

pub struct RemoteTerminalDispatcher;

#[async_trait]
impl TakeableCommandDispatcher for RemoteTerminalDispatcher {
    type Output = RemoteTerminal;
    type Request = ();

    fn key() -> String {
        RemoteTerminalHandler::key()
    }

    fn get_request(&self) -> Self::Request {
        ()
    }

    async fn dispatch(
        mut self,
        session: &mut Option<Session>,
        _: Self::Request,
    ) -> Result<Self::Output> {
        if let Some(session) = session.take() {
            debug!("starting remote terminal");

            let (input, mut input_recv) = mpsc::channel::<TerminalPacket>(10);
            let (output, output_recv) = mpsc::channel::<Vec<u8>>(10);

            let (mut read, mut write, _) = session.destructure();

            let mut joinset = JoinSet::new();

            joinset.spawn(async move {
                loop {
                    match read.read_object::<Vec<u8>>().await {
                        Ok(chunk) => {
                            if let Err(err) = output.send(chunk).await {
                                tracing::error!("{err}");
                            }
                        }
                        Err(err) => {
                            tracing::error!("{err}");
                            return;
                        }
                    }
                }
            });

            joinset.spawn(async move {
                while let Some(packet) = input_recv.recv().await {
                    if let Err(err) = write.write_object(&packet).await {
                        tracing::error!("{err}");
                        return;
                    }
                }
            });

            Ok(RemoteTerminal {
                input: input,
                output: tokio::sync::Mutex::new(output_recv),
                joinset,
            })
        } else {
            Err(anyhow!("tried dispatching command with None"))
        }
    }
}
