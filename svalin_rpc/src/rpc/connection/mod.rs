use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::task::JoinSet;
use tracing::{debug, error};

use crate::rpc::{command::HandlerCollection, session::Session};
use crate::transport::session_transport::{SessionTransportReader, SessionTransportWriter};

#[async_trait]
pub trait ConnectionBase: Send + Sync {
    async fn open_raw_session(
        &self,
    ) -> Result<(
        Box<dyn SessionTransportReader>,
        Box<dyn SessionTransportWriter>,
    )>;

    fn peer(&self) -> &Peer;

    async fn closed(&self);
}

use super::peer::Peer;

pub mod direct_connection;

#[async_trait]
pub trait Connection: ConnectionBase {
    async fn open_session(&self, command_key: String) -> Result<Session>;
}

#[async_trait]
impl<T> Connection for T
where
    T: ConnectionBase,
{
    async fn open_session(&self, command_key: String) -> Result<Session> {
        debug!("creating transport");

        let (read, write) = self.open_raw_session().await?;

        debug!("transport created, pass to session");

        let mut session = Session::new(read, write);

        debug!("requesting session");

        session.request_session(command_key).await?;

        debug!("session request successful");

        Ok(session)
    }
}

#[async_trait]
pub trait ServeableConnectionBase: ConnectionBase {
    async fn accept_raw_session(
        &self,
    ) -> Result<(
        Box<dyn SessionTransportReader>,
        Box<dyn SessionTransportWriter>,
    )>;
}

#[async_trait]
pub trait ServeableConnection {
    async fn serve(&self, commands: HandlerCollection) -> Result<()>;
}

#[async_trait]
impl<T> ServeableConnection for T
where
    T: ServeableConnectionBase,
{
    async fn serve(&self, commands: HandlerCollection) -> Result<()> {
        debug!("waiting for incoming data stream");
        let mut open_sessions = JoinSet::<()>::new();

        loop {
            match self.accept_raw_session().await {
                Ok((read, write)) => {
                    let mut session = Session::new(read, write);

                    let commands2 = commands.clone();
                    open_sessions.spawn(async move {
                        let res = session
                            .handle(commands2)
                            .await
                            .context("error handling session");
                        if let Err(e) = res {
                            // TODO: Actually handle Error
                            error!("{:?}", e);
                            #[cfg(test)]
                            {
                                panic!("{:?}", e);
                            }
                        }
                    });
                }
                Err(_err) => while open_sessions.join_next().await.is_some() {},
            }
        }
    }
}
