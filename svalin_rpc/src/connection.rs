use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio::task::JoinSet;
use tracing::{debug, error};

use crate::{session::SessionCreated, HandlerCollection, Session, SessionOpen};

#[async_trait]
pub trait Connection: Send + Sync {
    async fn serve(&self, commands: Arc<HandlerCollection>) -> Result<()>;

    async fn open_session(&self, command_key: String) -> Result<Session<SessionOpen>>;

    async fn closed(&self);
}

pub struct DirectConnection {
    conn: quinn::Connection,
}

#[async_trait]
impl crate::Connection for DirectConnection {
    async fn serve(&self, commands: Arc<HandlerCollection>) -> Result<()> {
        debug!("waiting for incoming data stream");
        let mut open_sessions = JoinSet::<()>::new();

        loop {
            match self.accept_session().await {
                Ok(session) => {
                    let commands2 = commands.clone();
                    open_sessions.spawn(async move {
                        let res = session.handle(commands2).await;
                        if let Err(e) = res {
                            // TODO: Actually handle Error
                            error!("{}", e);
                        }
                    });
                }
                Err(_err) => while open_sessions.join_next().await.is_some() {},
            }
        }
    }

    async fn open_session(&self, command_key: String) -> Result<Session<SessionOpen>> {
        let (send, recv) = self.conn.open_bi().await.map_err(|err| anyhow!(err))?;

        let session = Session::new(Box::new(recv), Box::new(send));

        let session = session.request_session(command_key).await?;

        Ok(session)
    }

    async fn closed(&self) {
        self.closed().await
    }
}

impl DirectConnection {
    pub(crate) fn new(conn: quinn::Connection) -> Self {
        DirectConnection { conn }
    }

    async fn accept_session(&self) -> Result<Session<SessionCreated>> {
        let (send, recv) = self.conn.accept_bi().await.map_err(|err| anyhow!(err))?;

        let session = Session::new(Box::new(recv), Box::new(send));

        Ok(session)
    }
}
