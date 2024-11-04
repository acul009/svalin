use std::sync::Arc;

use anyhow::{Ok, Result};
use futures::{future::try_join_all, stream, StreamExt};
use marmelade::{Data, Scope};
use svalin_pki::{
    get_current_timestamp,
    signed_object::{SignedObject, VerifiedObject},
    verifier::exact::ExactVerififier,
    Certificate,
};
use tokio::sync::broadcast;

use crate::shared::join_agent::PublicAgentData;

#[derive(Debug)]
pub struct AgentStore {
    scope: Scope,
    broadcast: broadcast::Sender<AgentUpdate>,
    verifier: ExactVerififier,
}

#[derive(Clone, Debug)]
pub enum AgentUpdate {
    Add(Arc<VerifiedObject<PublicAgentData>>),
}

impl AgentStore {
    pub fn open(scope: Scope, root: Certificate) -> Arc<Self> {
        let (broadcast, _) = broadcast::channel(10);
        Arc::new(Self {
            scope,
            broadcast,
            verifier: ExactVerififier::new(root),
        })
    }

    pub async fn get_agent(
        &self,
        fingerprint: &[u8; 32],
    ) -> Result<Option<SignedObject<PublicAgentData>>> {
        let mut agent: Option<SignedObject<PublicAgentData>> = None;
        self.scope.view(|b| {
            agent = b.get_object(&fingerprint[..])?;

            Ok(())
        })?;

        if let Some(agent) = agent {
            Ok(Some(
                // TODO: if the agent is invalid it should probably be removed and the a security
                // alert should be raised
                agent
                    .verify(&self.verifier, get_current_timestamp())
                    .await?
                    .pack_owned(),
            ))
        } else {
            Ok(agent)
        }
    }

    pub async fn add_agent(&self, agent: SignedObject<PublicAgentData>) -> Result<()> {
        let agent = agent
            .verify(&self.verifier, get_current_timestamp())
            .await?;

        self.scope.update(|b| {
            let key = agent.cert.get_fingerprint().to_vec();
            b.put_object(key, agent.pack())?;

            Ok(())
        })?;

        self.broadcast.send(AgentUpdate::Add(Arc::new(agent)))?;

        Ok(())
    }

    pub async fn list_agents(&self) -> Result<Vec<VerifiedObject<PublicAgentData>>> {
        let mut agents: Vec<SignedObject<PublicAgentData>> = Vec::new();

        self.scope.view(|b| {
            agents = b.list_objects()?;

            Ok(())
        })?;

        try_join_all(
            agents
                .into_iter()
                .map(|v| v.verify(&self.verifier, get_current_timestamp())),
        )
        .await
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentUpdate> {
        self.broadcast.subscribe()
    }
}
