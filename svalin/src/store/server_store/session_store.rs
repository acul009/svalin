use std::sync::Arc;

use svalin_pki::{
    Certificate, CertificateType, SpkiHash, UnverifiedCertificate, get_current_timestamp,
};

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
    pub async fn open(pool: sqlx::SqlitePool) -> Result<Arc<Self>, sqlx::Error> {
        // Existing databases need their certificate expiry extracted once.
        let mut tx = pool.begin().await?;
        let sessions = sqlx::query_as::<_, (Vec<u8>, Vec<u8>)>(
            "SELECT spki_hash, certificate FROM sessions WHERE expires_at IS NULL",
        )
        .fetch_all(&mut *tx)
        .await?;
        for (spki_hash, der) in sessions {
            let certificate = UnverifiedCertificate::from_der(der)
                .map_err(|err| sqlx::Error::Decode(Box::new(err)))?;
            sqlx::query("UPDATE sessions SET expires_at = ? WHERE spki_hash = ?")
                .bind(certificate.expires_at())
                .bind(spki_hash)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(Arc::new(Self { pool }))
    }

    pub async fn add_session(
        &self,
        certificate: Certificate,
    ) -> anyhow::Result<(), AddSessionError> {
        if certificate.certificate_type() != CertificateType::UserSession {
            return Err(AddSessionError::InvalidCertificateType(
                certificate.certificate_type(),
            ));
        }

        let spki_hash = certificate.spki_hash().as_slice();
        let issuer = certificate.issuer().as_slice();
        let der = certificate.as_der();

        sqlx::query(
            "INSERT INTO sessions (spki_hash, issuer, certificate, expires_at) VALUES (?, ?, ?, ?)",
        )
        .bind(spki_hash)
        .bind(issuer)
        .bind(der)
        .bind(certificate.expires_at())
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

    /// Deletes expired session certificates and returns the number removed.
    pub async fn delete_expired_sessions(&self) -> anyhow::Result<u64> {
        let now = i64::try_from(get_current_timestamp())?;
        let result = sqlx::query("DELETE FROM sessions WHERE expires_at < ?")
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn list_user_sessions(
        &self,
        user: &SpkiHash,
    ) -> anyhow::Result<Vec<UnverifiedCertificate>> {
        let spki_hash = user.as_slice();
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
