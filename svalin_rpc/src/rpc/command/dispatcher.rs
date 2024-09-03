use anyhow::{anyhow, Result};
use async_trait::async_trait;

use crate::rpc::session::Session;

#[async_trait]
pub trait CommandDispatcher: Send + Sync {
    type Output: Send;
    fn key(&self) -> String;
    async fn dispatch(self, session: &mut Session) -> Result<Self::Output>;
}

#[async_trait]
pub trait TakeableCommandDispatcher: Send + Sync {
    type Output: Send;
    fn key(&self) -> String;
    async fn dispatch(self, session: &mut Option<Session>) -> Result<Self::Output>;
}

#[async_trait]
impl<D> TakeableCommandDispatcher for D
where
    D: CommandDispatcher,
{
    type Output = D::Output;

    fn key(&self) -> String {
        self.key()
    }

    async fn dispatch(self, session: &mut Option<Session>) -> Result<Self::Output> {
        if let Some(session) = session {
            self.dispatch(session).await
            // self.dispatch(session).await
        } else {
            Err(anyhow!("tried dispatching command with None"))
        }
    }
}
