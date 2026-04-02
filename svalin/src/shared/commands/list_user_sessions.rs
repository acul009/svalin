use std::sync::Arc;

use async_trait::async_trait;
use svalin_pki::{SpkiHash, UnverifiedCertificate};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::Session,
};
use svalin_server_store::SessionStore;
use tokio_util::sync::CancellationToken;

pub struct ListUserSessionsHandler {
    session_store: Arc<SessionStore>,
}

#[async_trait]
impl CommandHandler for ListUserSessionsHandler {
    type Request = SpkiHash;

    fn key() -> String {
        "list-user-sessions".into()
    }
    async fn handle(
        &self,
        session: &mut Session,
        request: Self::Request,
        _cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let user_sessions = self.session_store.list_user_sessions(&request).await?;

        session.write_object(&user_sessions).await?;

        session.read_object::<()>().await?;

        Ok(())
    }
}

pub struct ListUserSessions<'a>(pub &'a SpkiHash);

impl<'a> CommandDispatcher for ListUserSessions<'a> {
    type Output = Vec<UnverifiedCertificate>;

    type Error = anyhow::Error;

    type Request = SpkiHash;

    fn key() -> String {
        ListUserSessionsHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        self.0
    }

    async fn dispatch(self, session: &mut Session) -> Result<Self::Output, Self::Error> {
        let user_sessions = session.read_object().await?;

        session.write_object(&()).await?;

        Ok(user_sessions)
    }
}
