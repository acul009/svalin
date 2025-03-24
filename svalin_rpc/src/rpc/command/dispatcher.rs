use async_trait::async_trait;
use serde::Serialize;
use thiserror::Error;

use crate::rpc::session::Session;

/// This is the default trait meant to be used to control the client side logic
/// of a command After executing the command, the session is properly closed
#[async_trait]
pub trait CommandDispatcher: Send + Sync {
    type Output: Send;
    type Error: Send;

    type Request: Send + Sync + Serialize;

    fn key() -> String;

    fn get_request(&self) -> Self::Request;

    async fn dispatch(
        self,
        session: &mut Session,
        request: Self::Request,
    ) -> Result<Self::Output, Self::Error>;
}

/// Some dispatchers may require taking ownership of the session.
/// This trait is meant to enable that.
/// If the session isn't taken, it will be properly closed
#[async_trait]
pub trait TakeableCommandDispatcher: Send + Sync {
    type Output: Send;
    type InnerError: Send;

    type Request: Send + Sync + Serialize;

    fn key() -> String;

    fn get_request(&self) -> Self::Request;

    async fn dispatch(
        self,
        session: &mut Option<Session>,
        request: Self::Request,
    ) -> Result<Self::Output, DispatcherError<Self::InnerError>>;
}

#[derive(Error, Debug)]
pub enum DispatcherError<Error> {
    #[error("tried dispatching command with None")]
    NoneSession,
    #[error("{0}")]
    Other(#[from] Error),
}

#[async_trait]
impl<D> TakeableCommandDispatcher for D
where
    D: CommandDispatcher,
{
    type Output = D::Output;
    type InnerError = D::Error;
    type Request = D::Request;

    fn key() -> String {
        Self::key()
    }

    fn get_request(&self) -> Self::Request {
        self.get_request()
    }

    async fn dispatch(
        self,
        session: &mut Option<Session>,
        request: Self::Request,
    ) -> Result<Self::Output, DispatcherError<Self::InnerError>> {
        if let Some(session) = session {
            Ok(self.dispatch(session, request).await?)
        } else {
            Err(DispatcherError::NoneSession)
        }
    }
}
