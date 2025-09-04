use svalin_pki::{Certificate, Fingerprint};

#[derive(Debug)]
pub struct SessionStore {
    pool: sqlx::SqlitePool,
}

impl SessionStore {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn add_session(&self, certificate: Certificate) -> Result<(), sqlx::Error> {
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
