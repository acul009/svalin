use std::sync::Arc;

use svalin_pki::{Certificate, CertificateType, Fingerprint};

use crate::server::user_store::UserStore;

#[derive(Debug)]
pub struct SessionStore {
    pool: sqlx::SqlitePool,
    user_store: Arc<UserStore>,
}

#[derive(Debug, thiserror::Error)]
pub enum AddSessionError {
    #[error("SQLx error: {0}")]
    SqlxError(#[from] sqlx::Error),
    #[error("Invalid certificate type: {0}")]
    InvalidCertificateType(CertificateType),
}

impl SessionStore {
    pub fn open(pool: sqlx::SqlitePool, user_store: Arc<UserStore>) -> Arc<Self> {
        Arc::new(Self { pool, user_store })
    }

    /// TODO: verify the certificate chain before acceping a session.
    pub async fn add_session(&self, certificate: Certificate) -> Result<(), AddSessionError> {
        if certificate.certificate_type() != CertificateType::UserDevice {
            return Err(AddSessionError::InvalidCertificateType(
                certificate.certificate_type(),
            ));
        }

        let fingerprint = certificate.fingerprint().as_slice();
        let issuer = certificate.issuer();
        let der = certificate.to_der();

        sqlx::query!(
            "INSERT INTO sessions (fingerprint, issuer, certificate) VALUES (?, ?, ?)",
            fingerprint,
            issuer,
            der
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_session(
        &self,
        fingerprint: &Fingerprint,
    ) -> Result<Option<Certificate>, anyhow::Error> {
        let fingerprint = fingerprint.as_slice();
        let session_der = sqlx::query_scalar!(
            "SELECT certificate FROM sessions WHERE fingerprint = ?",
            fingerprint
        )
        .fetch_optional(&self.pool)
        .await?;

        let session = match session_der {
            Some(der) => Certificate::from_der(der)?,
            None => return Ok(None),
        };

        Ok(Some(session))
    }
}
