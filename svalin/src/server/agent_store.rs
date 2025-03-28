use std::sync::Arc;

use anyhow::{Ok, Result};
use futures::future::try_join_all;
use svalin_pki::{
    Certificate, get_current_timestamp,
    signed_object::{SignedObject, VerifiedObject},
    verifier::exact::ExactVerififier,
};
use tokio::sync::broadcast;
use totp_rs::qrcodegen_image::image::EncodableLayout;

use crate::shared::join_agent::PublicAgentData;

const AGENT_PREFIX: &[u8] = b"agents/";

#[derive(Debug)]
pub struct AgentStore {
    tree: sled::Tree,
    broadcast: broadcast::Sender<AgentUpdate>,
    verifier: ExactVerififier,
}

#[derive(Clone, Debug)]
pub enum AgentUpdate {
    Add(Arc<VerifiedObject<PublicAgentData>>),
}

impl AgentStore {
    pub fn open(tree: sled::Tree, root: Certificate) -> Arc<Self> {
        let (broadcast, _) = broadcast::channel(10);
        Arc::new(Self {
            tree,
            broadcast,
            verifier: ExactVerififier::new(root),
        })
    }

    pub async fn get_agent(
        &self,
        fingerprint: &[u8; 32],
    ) -> Result<Option<SignedObject<PublicAgentData>>> {
        let mut key = AGENT_PREFIX.to_vec();
        key.extend(fingerprint);

        let agent = self
            .tree
            .get(&key)?
            .map(|agent| postcard::from_bytes::<SignedObject<PublicAgentData>>(&agent));

        match agent {
            None => Ok(None),
            Some(agent) => {
                let agent = agent?;

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

        let mut key = AGENT_PREFIX.to_vec();
        key.extend(agent.cert.fingerprint());

        self.tree
            .insert(key, postcard::to_extend(&agent.pack(), Vec::new())?)?;

        self.broadcast.send(AgentUpdate::Add(Arc::new(agent)))?;

        Ok(())
    }

    pub async fn list_agents(&self) -> Result<Vec<VerifiedObject<PublicAgentData>>> {
        try_join_all(self.tree.scan_prefix(AGENT_PREFIX).map(|v| async move {
            let (_, agent) = v?;
            let agent = postcard::from_bytes::<SignedObject<PublicAgentData>>(agent.as_bytes())?;

            Ok(agent
                .verify(&self.verifier, get_current_timestamp())
                .await?)
        }))
        .await
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentUpdate> {
        self.broadcast.subscribe()
    }
}
