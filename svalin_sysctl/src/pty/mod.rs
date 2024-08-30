use std::sync::LazyLock;

use anyhow::Result;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};

static SHELL: LazyLock<String> = LazyLock::new(|| CommandBuilder::new_default_prog().get_shell());

pub struct PtyProcess {
    master: Mutex<Box<dyn MasterPty + Send>>,
    write: mpsc::Sender<Vec<u8>>,
    read: Mutex<mpsc::Receiver<Vec<u8>>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TerminalSize {
    cols: u16,
    rows: u16,
}

impl From<TerminalSize> for PtySize {
    fn from(value: TerminalSize) -> Self {
        Self {
            rows: value.rows,
            cols: value.cols,
            ..Default::default()
        }
    }
}

impl PtyProcess {
    pub fn shell(size: TerminalSize) -> Result<Self> {
        let pty_system = native_pty_system();

        let pair = pty_system.openpty(size.into())?;

        let shell_cmd = CommandBuilder::new(SHELL.to_owned());

        let child = pair.slave.spawn_command(shell_cmd)?;

        let master = pair.master;
        let mut writer = master.take_writer()?;
        let (writer_send, mut writer_recv) = mpsc::channel::<Vec<u8>>(10);

        // writer task
        tokio::task::spawn_blocking(move || {
            while let Some(chunk) = writer_recv.blocking_recv() {
                if let Err(_) = writer.write_all(&chunk) {
                    return;
                }
            }
        });

        let mut reader = master.try_clone_reader()?;
        let (reader_send, reader_recv) = mpsc::channel::<Vec<u8>>(10);

        // reader task
        tokio::task::spawn_blocking(move || {
            let mut buffer = [0u8; 1024];
            while let Ok(bytes) = reader.read(&mut buffer) {
                let mut chunk = Vec::new();
                chunk.extend_from_slice(&buffer[0..bytes]);
                if let Err(_) = reader_send.blocking_send(chunk) {
                    return;
                }
            }
        });

        Ok(Self {
            master: Mutex::new(master),
            read: Mutex::new(reader_recv),
            write: writer_send,
        })
    }

    pub async fn resize(&self, size: TerminalSize) -> Result<()> {
        self.master.lock().await.resize(size.into())
    }

    pub async fn write(&self, content: Vec<u8>) -> Result<()> {
        self.write.send(content).await?;

        Ok(())
    }

    pub async fn read(&self) -> Option<Vec<u8>> {
        self.read.lock().await.recv().await
    }
}