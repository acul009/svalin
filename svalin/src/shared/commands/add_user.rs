use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use svalin_macros::rpc_dispatch;
use svalin_rpc::{Session, SessionOpen};

use crate::server::users::UserStore;

use super::public_server_status::PublicStatus;

pub struct AddUserHandler {
    userstore: Arc<UserStore>,
}

fn add_user_key() -> String {
    "add_user".to_owned()
}

#[async_trait]
impl svalin_rpc::CommandHandler for AddUserHandler {
    fn key(&self) -> String {
        add_user_key()
    }

    #[must_use]
    async fn handle(&self, mut session: Session<SessionOpen>) -> anyhow::Result<()> {
        todo!()
    }
}

#[rpc_dispatch(add_user_key())]
pub async fn get_public_status(session: &mut Session<SessionOpen>) -> Result<PublicStatus> {
    todo!()
}
