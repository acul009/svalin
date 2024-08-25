use anyhow::Result;
use async_trait::async_trait;
use svalin_rpc::rpc::{command::CommandHandler, session::Session};
use svalin_sysctl::pty::PtyProcess;
use tokio::sync::mpsc;

enum TerminalPacket {
    Close,
    Input(String),
    Resize { cols: u16, rows: u16 },
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
        let pty = PtyProcess::shell(rows, cols)
        loop {

        }
        todo!()
    }
}
