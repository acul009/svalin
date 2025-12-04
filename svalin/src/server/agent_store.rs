use std::sync::Arc;

use anyhow::{Ok, Result};
use futures::future::try_join_all;
use sqlx::SqlitePool;
use svalin_pki::{
    Certificate, ExactVerififier, SignedObject, SpkiHash, VerifiedObject, get_current_timestamp,
};
use tokio::sync::broadcast;

use crate::shared::join_agent::PublicAgentData;

#[derive(Debug)]
pub struct AgentStore {
    pool: SqlitePool,
    broadcast: broadcast::Sender<AgentUpdate>,
    verifier: ExactVerififier,
}

#[derive(Clone, Debug)]
pub enum AgentUpdate {
    Add(Arc<VerifiedObject<PublicAgentData>>),
}

impl AgentStore {
    pub fn open(pool: SqlitePool, root: Certificate) -> Arc<Self> {
        let (broadcast, _) = broadcast::channel(10);
        Arc::new(Self {
            pool,
            broadcast,
            verifier: ExactVerififier::new(root),
        })
    }

    pub async fn get_agent(
        &self,
        spki_hash: &SpkiHash,
    ) -> Result<Option<SignedObject<PublicAgentData>>> {
        let spki_hash = spki_hash.as_slice();

        let agent_data = sqlx::query!("SELECT data FROM agents WHERE spki_hash = ?", spki_hash)
            .fetch_optional(&self.pool)
            .await?;

        match agent_data {
            None => Ok(None),
            Some(agent_data) => {
                let agent: SignedObject<PublicAgentData> = postcard::from_bytes(&agent_data.data)?;

                Ok(Some(
                    agent
                        .verify(&self.verifier, get_current_timestamp())
                        .await?
                        .pack_owned(),
                ))
            }
        }
    }

    pub async fn add_agent(&self, agent: SignedObject<PublicAgentData>) -> Result<()> {
        let agent = agent
            .verify(&self.verifier, get_current_timestamp())
            .await?;

        let spki_hash = agent.cert.spki_hash();
        let spki_hash = spki_hash.as_slice();
        let agent_data = postcard::to_stdvec(agent.pack())?;

        sqlx::query!(
            "INSERT INTO agents (spki_hash, data) VALUES (?, ?)",
            spki_hash,
            agent_data
        )
        .execute(&self.pool)
        .await?;

        self.broadcast.send(AgentUpdate::Add(Arc::new(agent)))?;

        Ok(())
    }

    pub async fn list_agents(&self) -> Result<Vec<VerifiedObject<PublicAgentData>>> {
        let agent_data = sqlx::query!("SELECT data FROM agents")
            .fetch_all(&self.pool)
            .await?;

        try_join_all(agent_data.into_iter().map(|row| async move {
            let agent: SignedObject<PublicAgentData> = postcard::from_bytes(&row.data)?;

            agent.verify(&self.verifier, get_current_timestamp()).await
        }))
        .await
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentUpdate> {
        self.broadcast.subscribe()
    }
}
