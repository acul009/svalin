use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::task::JoinSet;
use tracing::{debug, error};

use crate::permissions::PermissionHandler;
use crate::rpc::{command::handler::HandlerCollection, session::Session};
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

use super::command::dispatcher::TakeableCommandDispatcher;
use super::peer::Peer;

pub mod direct_connection;

#[async_trait]
pub trait Connection: Sync {
    // async fn open_session(&self, command_key: String) -> Result<Session>;
    async fn dispatch<D: TakeableCommandDispatcher>(&self, dispatcher: D) -> Result<D::Output>;
}

#[async_trait]
impl<T> Connection for T
where
    T: ConnectionBase,
{
    // async fn open_session(&self, command_key: String) -> Result<Session> {
    //     debug!("creating transport");

    //     let (read, write) = self.open_raw_session().await?;

    //     debug!("transport created, pass to session");

    //     let mut session = Session::new(read, write, Peer::Anonymous);

    //     debug!("requesting session");

    //     session.request_session(command_key).await?;

    //     debug!("session request successful");

    //     Ok(session)
    // }

    async fn dispatch<D: TakeableCommandDispatcher>(&self, dispatcher: D) -> Result<D::Output> {
        let (read, write) = self.open_raw_session().await?;

        let session = Session::new(read, write, Peer::Anonymous);

        session.dispatch(dispatcher).await
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
pub trait ServeableConnection<P, Permission>
where
    P: PermissionHandler<Permission>,
{
    async fn serve(&self, commands: HandlerCollection<P, Permission>) -> Result<()>;
}

#[async_trait]
impl<T, P, Permission> ServeableConnection<P, Permission> for T
where
    T: ServeableConnectionBase,
    P: PermissionHandler<Permission>,
    Permission: 'static,
{
    async fn serve(&self, commands: HandlerCollection<P, Permission>) -> Result<()> {
        debug!("waiting for incoming data stream");
        let mut open_sessions = JoinSet::<()>::new();

        loop {
            match self.accept_raw_session().await {
                Ok((read, write)) => {
                    let session = Session::new(read, write, Peer::Anonymous);

                    let commands2 = commands.clone();
                    open_sessions.spawn(async move {
                        let commands2 = commands2;
                        let res = session
                            .handle(&commands2)
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
