use std::sync::Arc;

use svalin_pki::{Certificate, CertificateType, SpkiHash, UnverifiedCertificate};

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
    pub fn open(pool: sqlx::SqlitePool) -> Arc<Self> {
        Arc::new(Self { pool })
    }

    pub async fn add_session(
        &self,
        certificate: Certificate,
    ) -> anyhow::Result<(), AddSessionError> {
        if certificate.certificate_type() != CertificateType::UserDevice {
            return Err(AddSessionError::InvalidCertificateType(
                certificate.certificate_type(),
            ));
        }

        let spki_hash = certificate.spki_hash().as_slice();
        let issuer = certificate.issuer().as_slice();
        let der = certificate.as_der();

        sqlx::query!(
            "INSERT INTO sessions (spki_hash, issuer, certificate) VALUES (?, ?, ?)",
            spki_hash,
            issuer,
            der
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_session(
        &self,
        spki_hash: &SpkiHash,
    ) -> anyhow::Result<Option<UnverifiedCertificate>, anyhow::Error> {
        let fingerprint = spki_hash.as_slice();
        let session_der = sqlx::query_scalar!(
            "SELECT certificate FROM sessions WHERE spki_hash = ?",
            fingerprint
        )
        .fetch_optional(&self.pool)
        .await?;

        let session = match session_der {
            Some(der) => UnverifiedCertificate::from_der(der)?,
            None => return Ok(None),
        };

        Ok(Some(session))
    }

    pub async fn list_user_sessions(
        &self,
        user: &Certificate,
    ) -> anyhow::Result<Vec<UnverifiedCertificate>> {
        let spki_hash = user.spki_hash().as_slice();
        let session_ders = sqlx::query_scalar!(
            "SELECT certificate FROM sessions WHERE issuer = ?",
            spki_hash
        )
        .fetch_all(&self.pool)
        .await?;

        let sessions = session_ders
            .into_iter()
            .map(|der| UnverifiedCertificate::from_der(der))
            .collect::<Result<_, _>>()?;

        Ok(sessions)
    }
}
