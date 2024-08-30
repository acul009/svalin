use std::{future::Future, pin::pin, task::Poll};

use anyhow::Result;
use async_trait::async_trait;
use futures::{future::poll_fn, select, FutureExt};
use pin_project::pin_project;
use serde::{Deserialize, Serialize};
use svalin_rpc::rpc::{command::handler::CommandHandler, session::Session};
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

enum CopyState<A, B> {
    WaitOnRead(A),
    WaitOnWrite(B),
    Continue,
    Finished,
}

impl Future for CopyState<String, String> {}

pub struct RemoteTerminalHandler {}

impl RemoteTerminalHandler {}

#[async_trait]
impl CommandHandler for RemoteTerminalHandler {
    fn key(&self) -> String {
        remote_terminal_key()
    }

    async fn handle(&self, session: &mut Session) -> Result<()> {
        let size: TerminalSize = session.read_object().await?;
        let pty = PtyProcess::shell(size)?;

        let mut copy_from_pty = CopyState::Continue;
        let mut output_helper = Vec::new();

        poll_fn(|cx| -> Poll<()> {
            loop {
                match &mut copy_from_pty {
                    CopyState::Continue => {
                        copy_from_pty = CopyState::WaitOnRead(pty.read());
                    }
                    CopyState::WaitOnRead(read_future) => match read_future.poll_unpin(cx) {
                        Poll::Ready(Some(output)) => {
                            output_helper = output;
                            copy_from_pty =
                                CopyState::WaitOnWrite(session.write_object(&output_helper));
                        }
                        Poll::Ready(None) => todo!(),
                        Poll::Pending => break,
                    },
                    CopyState::WaitOnWrite(write_future) => match write_future.poll(cx) {
                        Poll::Ready(Ok(_)) => {
                            copy_from_pty = CopyState::Continue;
                        }
                        Poll::Ready(Err(err)) => todo!(),
                        Poll::Pending => break,
                    },
                    CopyState::Finished => break,
                }
            }

            let read_pty = pin!(pty.read()).poll_unpin(cx);
            let read_session = pin!(session.read_object::<TerminalPacket>()).poll_unpin(cx);

            let mut pending = false;

            match read_pty {
                Poll::Ready(output) => {
                    match output {
                        Some(output) => match pin!(session.write_object(&output)).poll_unpin(cx) {
                            Poll::Ready(_) => (),
                            Poll::Pending => todo!(),
                        },
                        None => todo!(),
                    }

                    todo!();
                }
                Poll::Pending => pending = true,
            };

            match read_session {
                Poll::Ready(packet) => match packet {
                    Ok(TerminalPacket::Input(input)) => {
                        todo!();
                    }
                    Ok(TerminalPacket::Resize(size)) => {
                        todo!();
                    }
                    Ok(TerminalPacket::Close) => {
                        todo!();
                    }
                    Err(err) => todo!(),
                },
                Poll::Pending => pending = true,
            };

            if pending {
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        })
        .await;

        todo!()
        // loop {
        //     let mut output_future = pin!(pty.read().fuse());
        //     let mut input_future =
        // pin!(session.read_object::<TerminalPacket>().fuse());
        //     select! {
        //         output = output_future => {
        //             match output {
        //                 Some(output) => {
        //                     session.write_object(&output).await?;
        //                 },
        //                 None => {return Ok(())}
        //             }
        //         },
        //         input = input_future => {
        //             match input {
        //                 Ok(TerminalPacket::Input(input)) => {
        //                     pty.write(input).await;
        //                 },
        //                 Ok(TerminalPacket::Resize(size)) => {
        //                     pty.resize(size);
        //                 },
        //                 Ok(TerminalPacket::Close) => {
        //                     return Ok(());
        //                 },
        //                 Err(err) => return Err(err),
        //             }
        //         },
        //     }
        // }
    }
}
