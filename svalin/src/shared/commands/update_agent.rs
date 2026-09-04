use std::path::PathBuf;

use anyhow::{Context, anyhow};
use async_trait::async_trait;
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::Session,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    select,
    sync::{mpsc, oneshot},
};
use tokio_util::sync::CancellationToken;

pub struct UpdateAgentHandler {
    mutex: tokio::sync::Mutex<()>,
}

impl UpdateAgentHandler {
    pub fn new() -> Self {
        Self {
            mutex: tokio::sync::Mutex::new(()),
        }
    }
}

#[async_trait]
impl CommandHandler for UpdateAgentHandler {
    type Request = ();

    fn key() -> String {
        "update-agent".into()
    }

    async fn handle(
        &self,
        session: &mut Session,
        _request: Self::Request,
        cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let Ok(_guard) = self.mutex.try_lock() else {
            let err = Result::<(), String>::Err("update already in progress".into());
            session.write_object(&err).await?;
            return Ok(());
        };

        let (mut send, recv) = tokio::io::duplex(1024 * 4);
        let (result_send, prepared) = oneshot::channel();
        tokio::spawn(async move {
            let result = crate::installer::prepare_update_agent(recv).await;
            let _ = result_send.send(result);
        });

        loop {
            let chunk = session.read_chunk().await?;
            session.write_object::<Result<(), String>>(&Ok(())).await?;
            if chunk.is_empty() {
                send.flush().await?;
                send.shutdown().await?;
                break;
            }
            send.write_all(&chunk).await?;
        }

        let prepared = prepared.await??;

        select! {
            update_result = crate::installer::update_agent(&prepared) => {
                let update_result = update_result.context("error while executing update");
                let send_result: Result<(), String> = match &update_result {
                    Ok(_) => Ok(()),
                    Err(e) => Err(e.to_string()),
                };
                session.write_object(&send_result).await?;
                update_result
            }
            _ = cancel.cancelled() => {
                let send_result: Result<(), String> = Ok(());
                session.write_object(&send_result).await?;
                Ok(())
            }
        }
    }
}

pub struct UpdateAgent {
    pub file: PathBuf,
    pub progress: mpsc::Sender<f32>,
}

impl CommandDispatcher for UpdateAgent {
    type Output = ();

    type Error = anyhow::Error;

    type Request = ();

    fn key() -> String {
        UpdateAgentHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &()
    }

    async fn dispatch(self, session: &mut Session) -> Result<Self::Output, Self::Error> {
        let mut source = tokio::fs::File::open(&self.file).await?;
        #[cfg(target_os = "linux")]
        let size = {
            use std::os::unix::fs::MetadataExt;
            source.metadata().await?.size() as f32
        };
        #[cfg(target_os = "windows")]
        let size = {
            use std::os::windows::fs::MetadataExt;

            source.metadata().await?.file_size() as f32
        };

        let mut progress = 0.0;
        loop {
            let mut buffer = Box::new([0u8; 1024 * 64]);
            let read = source.read(buffer.as_mut()).await?;
            session.write_chunk(&buffer.as_slice()[..read]).await?;
            if let Err(err) = session.read_object::<Result<(), String>>().await? {
                return Err(anyhow!("peer update error: {}", err));
            }
            progress += read as f32;
            self.progress.send(progress / size).await?;
            if read == 0 {
                break;
            }
        }

        if let Err(err) = session.read_object::<Result<(), String>>().await? {
            return Err(anyhow!("peer update error: {}", err));
        }

        Ok(())
    }
}
