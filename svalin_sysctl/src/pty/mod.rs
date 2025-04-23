use std::{
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};

use anyhow::Result;
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system, win::WinChild};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

static SHELL: LazyLock<String> = LazyLock::new(|| {
    let shell = CommandBuilder::new_default_prog().get_shell();
    if &shell == "cmd.exe" {
        return "powershell.exe".to_string();
    }

    shell
});

pub enum TerminalInput {
    Input(Vec<u8>),
    Resize(TerminalSize),
}

#[derive(Debug)]
pub struct PtyProcess {
    write: mpsc::Sender<TerminalInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalSize {
    pub cols: u16,
    pub rows: u16,
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
    pub async fn shell(size: TerminalSize) -> Result<(Self, mpsc::Receiver<Vec<u8>>)> {
        tokio::task::spawn_blocking(move || {
            let pty_system = native_pty_system();

            let pair = pty_system.openpty(size.into())?;

            let shell_cmd = CommandBuilder::new(SHELL.to_owned());

            let child = pair.slave.spawn_command(shell_cmd)?;
            drop(pair.slave);

            let master = pair.master;
            let mut reader = master.try_clone_reader()?;
            let (writer_send, writer_recv) = mpsc::channel::<TerminalInput>(10);
            let (helper_send, helper_recv) = mpsc::channel::<TerminalInput>(10);

            let cancel = CancellationToken::new();
            let cancel2 = cancel.clone();

            tokio::spawn(async move {
                let cancel = cancel2;
                let mut writer_recv = writer_recv;
                cancel
                    .run_until_cancelled(async move {
                        while let Some(input) = writer_recv.recv().await {
                            if helper_send.send(input).await.is_err() {
                                break;
                            }
                        }
                    })
                    .await;
                println!("helper task exited");
            });

            // writer thread
            std::thread::spawn(move || {
                let mut helper_recv = helper_recv;
                if let Ok(mut writer) = master.take_writer() {
                    while let Some(input) = helper_recv.blocking_recv() {
                        match input {
                            TerminalInput::Input(input) => {
                                // println!("writer thread received {} bytes", input.len());
                                if let Err(err) = writer.write_all(&input) {
                                    panic!("{}", err);
                                    return;
                                }
                            }
                            TerminalInput::Resize(size) => {
                                // println!("writer thread received resize");
                                if let Err(err) = master.resize(size.into()) {
                                    panic!("{}", err);
                                    return;
                                }
                            }
                        }
                    }
                }
                println!("Writer task exited");
            });

            let (reader_send, reader_recv) = mpsc::channel::<Vec<u8>>(10);

            // reader thread
            let reader_thread = std::thread::spawn(move || {
                println!("async reader started");
                let mut buffer = [0u8; 1024];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(bytes) => {
                            // println!(
                            //     "reader thread received {} bytes: {:?}",
                            //     bytes,
                            //     &buffer[0..bytes]
                            // );
                            if bytes == 0 {
                                break;
                            }

                            let mut chunk = Vec::new();
                            chunk.extend_from_slice(&buffer[0..bytes]);
                            if let Err(err) = reader_send.try_send(chunk) {
                                println!("reader thread error: {}", err);
                                panic!("{}", err);
                                return;
                            }
                        }
                        Err(err) => {
                            println!("reader task error: {}", err);
                            panic!("{}", err);
                            return;
                        }
                    }
                }
                println!("Reader task exited");
            });

            let win_child = child.as_any().downcast_ref::<WinChild>();
            match win_child {
                None => {
                    std::thread::spawn(move || {
                        let mut child = child;
                        let _ = child.wait();
                        println!("child finished");

                        cancel.cancel();
                    });
                }
                Some(_win_child) => {
                    tokio::spawn(async move {
                        println!("win child detected");
                        let mut child = child;
                        let win_child = child.as_any_mut().downcast_mut::<WinChild>().unwrap();

                        let _ = win_child.await;

                        println!("child finished");

                        cancel.cancel();
                    });
                }
            }

            Ok((Self { write: writer_send }, reader_recv))
        })
        .await?
    }

    pub async fn resize(&self, size: TerminalSize) -> Result<()> {
        self.write.send(TerminalInput::Resize(size)).await?;

        Ok(())
    }

    pub fn try_resize(&self, size: TerminalSize) -> Result<()> {
        self.write.try_send(TerminalInput::Resize(size))?;

        Ok(())
    }

    pub async fn write(&self, content: Vec<u8>) -> Result<()> {
        self.write.send(TerminalInput::Input(content)).await?;

        Ok(())
    }

    pub fn try_write(&self, content: Vec<u8>) -> Result<()> {
        self.write.try_send(TerminalInput::Input(content))?;

        Ok(())
    }
}
