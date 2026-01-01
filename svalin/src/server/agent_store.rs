use std::sync::Arc;

use anyhow::{Ok, Result, anyhow};
use sqlx::SqlitePool;
use svalin_pki::{Certificate, SpkiHash, UnverifiedCertificate};
use tokio::sync::broadcast;

#[derive(Debug)]
pub struct AgentStore {
    pool: SqlitePool,
    broadcast: broadcast::Sender<AgentUpdate>,
}

#[derive(Clone, Debug)]
pub enum AgentUpdate {
    Add(Certificate),
}

impl AgentStore {
    pub fn open(pool: SqlitePool) -> Arc<Self> {
        let (broadcast, _) = broadcast::channel(10);
        Arc::new(Self { pool, broadcast })
    }

    pub async fn get_agent(&self, spki_hash: &SpkiHash) -> Result<Option<UnverifiedCertificate>> {
        let spki_hash = spki_hash.as_slice();

        let agent_data = sqlx::query_scalar!(
            "SELECT certificate FROM agents WHERE spki_hash = ?",
            spki_hash
        )
        .fetch_optional(&self.pool)
        .await?;

        match agent_data {
            None => Ok(None),
            Some(der) => Ok(Some(UnverifiedCertificate::from_der(der)?)),
        }
    }

    pub async fn add_agent(&self, agent: Certificate) -> Result<()> {
        let spki_hash = agent.spki_hash().as_slice();
        let certificate = agent.as_der();

        sqlx::query!(
            "INSERT INTO agents (spki_hash, certificate) VALUES (?, ?)",
            spki_hash,
            certificate
        )
        .execute(&self.pool)
        .await?;

        self.broadcast.send(AgentUpdate::Add(agent))?;

        Ok(())
    }

    pub async fn list_agents(&self) -> Result<Vec<UnverifiedCertificate>> {
        let agent_data = sqlx::query_scalar!("SELECT certificate FROM agents")
            .fetch_all(&self.pool)
            .await?;

        let certificates = agent_data
            .into_iter()
            .map(|certificate| {
                UnverifiedCertificate::from_der(certificate).map_err(|err| anyhow!(err))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(certificates)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentUpdate> {
        self.broadcast.subscribe()
    }
}
