use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_pki::{ArgonParams, CertificateType, EncryptedCredential};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    peer::Peer,
    session::Session,
};
use svalin_server_store::UserStore;
use tokio_util::sync::CancellationToken;

#[derive(Serialize, Deserialize)]
pub struct UserCredential {
    pub credential: EncryptedCredential,
    pub params: ArgonParams,
}

pub struct GetUserCredentialHandler {
    pub store: Arc<UserStore>,
}

#[async_trait]
impl CommandHandler for GetUserCredentialHandler {
    type Request = ();

    fn key() -> String {
        "get-user-credential".into()
    }
    async fn handle(
        &self,
        session: &mut Session,
        _request: Self::Request,
        _cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let Peer::Certificate(cert) = session.peer() else {
            anyhow::bail!("invalid peer");
        };
        if cert.certificate_type() != CertificateType::UserSession {
            anyhow::bail!("invalid certificate type");
        }

        let Some(user) = self.store.get_user(cert.issuer()).await? else {
            anyhow::bail!("user not found");
        };

        let response = UserCredential {
            credential: user.encrypted_credential,
            params: user.credential_key_params,
        };

        session.write_object(&response).await?;

        Ok(())
    }
}

pub struct GetUserCredential;

impl CommandDispatcher for GetUserCredential {
    type Output = UserCredential;

    type Error = anyhow::Error;

    type Request = ();

    fn key() -> String {
        GetUserCredentialHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &()
    }

    async fn dispatch(self, session: &mut Session) -> Result<Self::Output, Self::Error> {
        Ok(session.read_object().await?)
    }
}
