use std::pin::pin;

use anyhow::Result;
use async_trait::async_trait;
use futures::{select, FutureExt};
use serde::{Deserialize, Serialize};
use svalin_rpc::rpc::{command::CommandHandler, session::Session};
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

struct RemoteTerminalHandler {}

impl RemoteTerminalHandler {}

#[async_trait]
impl CommandHandler for RemoteTerminalHandler {
    fn key(&self) -> String {
        remote_terminal_key()
    }

    async fn handle(&self, session: &mut Session) -> Result<()> {
        let size: TerminalSize = session.read_object().await?;
        let mut pty = PtyProcess::shell(size)?;
        loop {
            let mut output_future = pin!(pty.read().fuse());
            let mut input_future = pin!(session.read_object::<TerminalPacket>().fuse());
            select! {
                output = output_future => {
                    match output {
                        Some(output) => {
                            session.write_object(&output).await?;
                        },
                        None => {return Ok(())}
                    }
                },
                input = input_future => {
                    match input {
                        Ok(TerminalPacket::Input(input)) => {
                            pty.write(input).await;
                        },
                        Ok(TerminalPacket::Resize(size)) => {
                            pty.resize(size);
                        },
                        Ok(TerminalPacket::Close) => {
                            return Ok(());
                        },
                        Err(err) => return Err(err),
                    }
                },
            }
        }
    }
}
