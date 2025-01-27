use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use tracing::debug;

use crate::{
    permissions::PermissionHandler,
    rpc::{
        command::{
            dispatcher::TakeableCommandDispatcher,
            handler::{HandlerCollection, TakeableCommandHandler},
        },
        peer::Peer,
        session::Session,
    },
};

pub struct DeauthenticateHandler<P>
where
    P: PermissionHandler,
{
    handler_collection: HandlerCollection<P>,
}

impl<P> DeauthenticateHandler<P>
where
    P: PermissionHandler,
{
    pub fn new(handler_collection: HandlerCollection<P>) -> Self {
        Self { handler_collection }
    }
}

fn deauth_key() -> String {
    "deauthenticate".into()
}

#[async_trait]
impl<P> TakeableCommandHandler for DeauthenticateHandler<P>
where
    P: PermissionHandler,
{
    type Request = ();

    fn key() -> String {
        deauth_key()
    }

    async fn handle(
        &self,
        session: &mut Option<Session>,
        _request: Self::Request,
        cancel: CancellationToken,
    ) -> Result<()> {
        if let Some(session) = session.take() {
            let (read, write, _) = session.destructure_transport();

            let session2 = Session::new(read, write, Peer::Anonymous);

            debug!("session deauthenticated, handing to next handler");

            session2.handle(&self.handler_collection, cancel).await
        } else {
            Err(anyhow!("Handler is missing the required session"))
        }
    }
}

#[derive(Default)]
pub struct Deauthenticate;

#[async_trait]
impl TakeableCommandDispatcher for Deauthenticate {
    type Output = Session;

    type Request = ();

    fn key() -> String {
        deauth_key()
    }

    fn get_request(&self) -> Self::Request {
        ()
    }

    async fn dispatch(
        self,
        session: &mut Option<Session>,
        _: Self::Request,
    ) -> Result<Self::Output> {
        if let Some(session) = session.take() {
            Ok(session)
        } else {
            Err(anyhow!("no session given"))
        }
    }
}
