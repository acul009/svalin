use std::{future::Future, pin::pin, task::Poll};

use anyhow::{anyhow, Ok, Result};
use async_trait::async_trait;
use futures::{channel::oneshot, future::poll_fn, select, FutureExt};
use serde::{Deserialize, Serialize};
use svalin_rpc::rpc::{
    command::{
        dispatcher::CommandDispatcher,
        handler::{CommandHandler, TakeableCommandHandler},
    },
    session::Session,
};
use svalin_sysctl::pty::{PtyProcess, TerminalSize};
use tokio::sync::mpsc;

#[derive(Debug, Serialize, Deserialize)]
enum TerminalPacket {
    Close,
    Input(Vec<u8>),
    Resize(TerminalSize),
}

pub struct RemoteTerminal {
    write: mpsc::Sender<TerminalPacket>,
    read: mpsc::Receiver<String>,
}

fn remote_terminal_key() -> String {
    "remote-terminal".into()
}

pub struct RemoteTerminalHandler {}

impl RemoteTerminalHandler {}

#[async_trait]
impl TakeableCommandHandler for RemoteTerminalHandler {
    fn key(&self) -> String {
        remote_terminal_key()
    }

    async fn handle(&self, session: &mut Option<Session>) -> Result<()> {
        if let Some(mut session) = session.take() {
            let size: TerminalSize = session.read_object().await?;
            let (pty, mut pty_reader) = PtyProcess::shell(size)?;

            let (mut read, mut write, _) = session.destructure();

            tokio::spawn(async move {
                loop {
                    let output = pty_reader.recv().await;
                    match output {
                        Some(output) => {
                            write.write_object(&output).await;
                        }
                        None => {
                            write.shutdown().await;
                            return;
                        }
                    }
                }
            });

            loop {
                let packet: TerminalPacket = read.read_object().await?;
                match packet {
                    TerminalPacket::Close => {
                        pty.close();
                        return Ok(());
                    }
                    TerminalPacket::Input(input) => {
                        pty.write(input).await;
                    }
                    TerminalPacket::Resize(size) => pty.resize(size).unwrap(),
                };
            }
        } else {
            Err(anyhow!("tried executing commandhandler with None"))
        }
    }
}

struct RemoteTerminalDispatcher;

#[async_trait]
impl CommandDispatcher for RemoteTerminalDispatcher {
    type Output = ();

    fn key(&self) -> String {
        remote_terminal_key()
    }

    async fn dispatch(self, session: &mut Session) -> Result<Self::Output> {
        todo!()
    }
}
