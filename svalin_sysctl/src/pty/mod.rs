use std::{process::Child, sync::LazyLock};

use anyhow::Result;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tokio::sync::mpsc;

static shell: LazyLock<String> = LazyLock::new(|| CommandBuilder::new_default_prog().get_shell());

pub struct PtyProcess {
    write: mpsc::Sender<Vec<u8>>,
    read: mpsc::Receiver<Vec<u8>>,
}

pub fn run_shell_in_pty(rows: u16, cols: u16) -> Result<()> {
    let pty_system = native_pty_system();

    let pair = pty_system.openpty(PtySize {
        cols: cols,
        rows: rows,
        ..Default::default()
    })?;

    let shell_cmd = CommandBuilder::new(shell.to_owned());

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
    let (reader_send, reader_write) = mpsc::channel::<Vec<u8>>(10);

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

    todo!()
}
