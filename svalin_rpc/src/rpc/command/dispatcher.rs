use anyhow::Result;
use async_trait::async_trait;

use crate::rpc::session::Session;

#[async_trait]
pub trait CommandDispatcher<T> {
    fn key(&self) -> String;
    async fn dispatch(self, session: &mut Session) -> Result<T>;
}
