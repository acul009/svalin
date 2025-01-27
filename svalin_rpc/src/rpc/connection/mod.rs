use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::select;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{debug, error};

use crate::permissions::PermissionHandler;
use crate::rpc::{command::handler::HandlerCollection, session::Session};
use crate::transport::session_transport::{SessionTransportReader, SessionTransportWriter};

#[async_trait]
pub trait ConnectionBase: Send + Sync + Clone {
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
pub trait Connection: Send + Sync + Clone {
    // async fn open_session(&self, command_key: String) -> Result<Session>;
    async fn dispatch<D: TakeableCommandDispatcher>(&self, dispatcher: D) -> Result<D::Output>;
    fn peer(&self) -> &Peer;
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

        let session = Session::new(read, write, self.peer().clone());

        session.dispatch(dispatcher).await
    }

    fn peer(&self) -> &Peer {
        self.peer()
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

    async fn close(&self);
}

#[async_trait]
pub trait ServeableConnection<P>
where
    P: PermissionHandler,
{
    async fn serve(&self, commands: HandlerCollection<P>, cancel: CancellationToken) -> Result<()>;
}

#[async_trait]
impl<T, P> ServeableConnection<P> for T
where
    T: ServeableConnectionBase,
    P: PermissionHandler,
    P::Permission: 'static,
{
    async fn serve(&self, commands: HandlerCollection<P>, cancel: CancellationToken) -> Result<()> {
        debug!("waiting for incoming data stream");
        let open_sessions = TaskTracker::new();

        loop {
            let cancel2 = cancel.clone();
            select! {
                _ = cancel.cancelled() => {
                    debug!("canceling connection serve loop");
                    break;
                }
                session = self.accept_raw_session() => {
                    match session {
                        Ok((read, write)) => {
                            let session = Session::new(read, write, self.peer().clone());

                            let commands2 = commands.clone();
                            open_sessions.spawn(async move {
                                let commands2 = commands2;
                                let res = session
                                    .handle(&commands2, cancel2)
                                    .await
                                    .context("error handling session");
                                if let Err(e) = res {
                                    // TODO: Actually handle Error
                                    error!("{:?}", e);
                                    #[cfg(test)]
                                    {
                                        use crate::permissions::PermissionCheckError;
                                        let mut chain = e.chain();
                                        chain.next(); // error handling session
                                        chain.next(); // error handling session with key
                                        if let Some(err) = chain.next() {
                                            if let Some(err) = err.downcast_ref::<PermissionCheckError>() {
                                                if let PermissionCheckError::PermissionDenied(_) = err {
                                                    return; // permission errors should
                                                            // not crash during tests
                                                            // since the client will be
                                                            // notified anyway
                                                }
                                            }
                                        }
                                        // all other errors should crash, so the test fails
                                        panic!("{:?}", e);
                                    }
                                }
                            });
                        },
                        Err(e) => {
                            error!("error accepting session: {}", e);
                            break;
                        }
                    }
                }
            }
        }

        open_sessions.close();
        open_sessions.wait().await;
        self.close().await;
        Ok(())
    }
}
