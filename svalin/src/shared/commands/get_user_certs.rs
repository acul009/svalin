use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_pki::{
    Certificate, CertificateType, SignatureVerificationError, SpkiHash, UnverifiedCertificate,
    UnverifiedCertificateChain, VerificationError, VerifyChainError, get_current_timestamp,
};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::{Session, SessionReadError},
};
use tokio_util::sync::CancellationToken;

use crate::server::{session_store::SessionStore, user_store::UserStore};

#[derive(Serialize, Deserialize)]
pub struct UnverifiedUserCertificates {
    pub cert_chain: UnverifiedCertificateChain,
    pub session_certs: Vec<UnverifiedCertificate>,
}

pub struct UserCertificates {
    pub user_cert: Certificate,
    pub session_certs: Vec<Certificate>,
}

#[derive(Debug, thiserror::Error)]
pub enum VerifyUserCertificatesError {
    #[error("user certificate chain error")]
    ChainError(#[from] VerifyChainError),
    #[error("session certificate error")]
    SessionError(#[from] SignatureVerificationError),
    #[error("wrong user certificate type")]
    WrongUserCertType,
    #[error("wrong session certificate type")]
    WrongSessionCertType,
}

impl UnverifiedUserCertificates {
    fn verify(self, root: &Certificate) -> Result<UserCertificates, VerifyUserCertificatesError> {
        let chain = self.cert_chain.verify(root, get_current_timestamp())?;
        let user_cert = chain.take_leaf();

        match user_cert.certificate_type() {
            CertificateType::Root | CertificateType::User => (),
            _ => return Err(VerifyUserCertificatesError::WrongUserCertType),
        }

        let session_certs = self
            .session_certs
            .into_iter()
            .map(|session| {
                let session = session.verify_signature(&user_cert, get_current_timestamp())?;
                match session.certificate_type() {
                    CertificateType::UserDevice => Ok(session),
                    _ => return Err(VerifyUserCertificatesError::WrongSessionCertType),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(UserCertificates {
            user_cert,
            session_certs,
        })
    }
}

impl UserCertificates {
    pub fn to_vec(self) -> Vec<Certificate> {
        let mut vec = self.session_certs;
        vec.push(self.user_cert);
        vec
    }
}

pub struct GetUserKeyPackages<'a>(pub &'a SpkiHash);

#[derive(Debug, thiserror::Error)]
pub enum GetUserKeyPackagesError {
    #[error(
        "The server has sent user certificates which do not match the requested user or are not valid"
    )]
    InvalidDataReceivedFromServer,
    #[error("The server encountered an error loading the user certificates")]
    ServerError,
    #[error("error reading server response: {0}")]
    SessionReadError(#[from] SessionReadError),
}

impl<'a> CommandDispatcher for GetUserKeyPackages<'a> {
    type Output = UnverifiedUserCertificates;

    type Error = GetUserKeyPackagesError;

    type Request = SpkiHash;

    fn key() -> String {
        "get-user-certificates".to_string()
    }

    fn get_request(&self) -> &Self::Request {
        self.0
    }

    async fn dispatch(
        self,
        session: &mut svalin_rpc::rpc::session::Session,
    ) -> Result<Self::Output, Self::Error> {
        let response: Result<UnverifiedUserCertificates, ()> = session.read_object().await?;
        match response {
            Err(()) => Err(GetUserKeyPackagesError::ServerError),
            Ok(certs) => certs
                .verify()
                .map_err(|_| GetUserKeyPackagesError::InvalidDataReceivedFromServer),
        }
    }
}

pub struct GetUserCertificateHandler {
    user_store: Arc<UserStore>,
    session_store: Arc<SessionStore>,
}

#[async_trait]
impl CommandHandler for GetUserCertificateHandler {
    type Request = SpkiHash;

    fn key() -> String {
        GetUserKeyPackages::key()
    }

    async fn handle(
        &self,
        session: &mut Session,
        request: Self::Request,
        _cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let user = match self.user_store.get_cert_by_spki_hash(&request).await {
            Ok(Some(user)) => user,
            Ok(None) => {
                let _ = session
                    .write_object(&Result::<UnverifiedUserCertificates, ()>::Err(()))
                    .await;
                return Ok(());
            }
            Err(err) => {
                let _ = session
                    .write_object(&Result::<UnverifiedUserCertificates, ()>::Err(()))
                    .await;
                return Err(err);
            }
        };

        let sessions = match self.session_store.list_user_sessions(&user).await {
            Ok(sessions) => sessions,
            Err(err) => {
                let _ = session
                    .write_object(&Result::<UnverifiedUserCertificates, ()>::Err(()))
                    .await;
                return Err(err);
            }
        };

        let certs: Result<_, ()> = Ok(UnverifiedUserCertificates {
            cert_chain: user,
            session_certs: sessions,
        });

        session.write_object(&certs).await?;

        Ok(())
    }
}
