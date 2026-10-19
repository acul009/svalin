use std::sync::Arc;

use crate::store::server_store::SessionStore;
use async_trait::async_trait;
use svalin_pki::{CertificateType, UnverifiedCertificate};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    peer::Peer,
    session::Session,
};
use tokio_util::sync::CancellationToken;

pub struct ListUserSessionsHandler {
    pub session_store: Arc<SessionStore>,
}

#[async_trait]
impl CommandHandler for ListUserSessionsHandler {
    type Request = ();

    fn key() -> String {
        "list-user-sessions".into()
    }
    async fn handle(
        &self,
        session: &mut Session,
        _request: Self::Request,
        _cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let Peer::Certificate(certificate) = session.peer() else {
            anyhow::bail!("Only user sessions can list their own sessions");
        };
        if certificate.certificate_type() != CertificateType::UserSession {
            anyhow::bail!("Only user sessions can list their own sessions");
        }
        let user_sessions = self
            .session_store
            .list_user_sessions(certificate.issuer())
            .await?;

        session.write_object(&user_sessions).await?;

        session.read_object::<()>().await?;

        Ok(())
    }
}

/// Lists sessions belonging to the authenticated session's user.
pub struct ListUserSessions;

impl CommandDispatcher for ListUserSessions {
    type Output = Vec<UnverifiedCertificate>;

    type Error = anyhow::Error;

    type Request = ();

    fn key() -> String {
        ListUserSessionsHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &()
    }

    async fn dispatch(self, session: &mut Session) -> Result<Self::Output, Self::Error> {
        let user_sessions = session.read_object().await?;

        session.write_object(&()).await?;

        Ok(user_sessions)
    }
}
