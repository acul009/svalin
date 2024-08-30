use anyhow::{anyhow, Result};
use async_trait::async_trait;

use crate::rpc::session::Session;

pub trait CommandDispatcher<T>: Send {
    fn key(&self) -> String;
    async fn dispatch(&self, session: &mut Session) -> Result<T>;
}

pub trait TakeableCommandDispatcher<T>: Send {
    fn key(&self) -> String;
    async fn dispatch(&self, session: &mut Option<Session>) -> Result<T>;
}

impl<D, T> TakeableCommandDispatcher<T> for D
where
    D: CommandDispatcher<T>,
{
    fn key(&self) -> String {
        self.key()
    }

    async fn dispatch(&self, session: &mut Option<Session>) -> Result<T> {
        if let Some(session) = session {
            self.dispatch(session).await
            // self.dispatch(session).await
        } else {
            Err(anyhow!("tried dispatching command with None"))
        }
    }
}
