use svalin_pki::{Certificate, CertificateType, Fingerprint};

#[derive(Debug)]
pub struct SessionStore {
    pool: sqlx::SqlitePool,
}

#[derive(Debug, thiserror::Error)]
pub enum AddSessionError {
    #[error("SQLx error: {0}")]
    SqlxError(#[from] sqlx::Error),
    #[error("Invalid certificate type: {0}")]
    InvalidCertificateType(CertificateType),
}

impl SessionStore {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }

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
        fingerprint: Fingerprint,
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
