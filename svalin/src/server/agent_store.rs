use std::sync::Arc;

use anyhow::{Ok, Result};
use marmelade::{Data, Scope};
use svalin_pki::{signed_object::SignedObject, Certificate};
use tokio::sync::broadcast;

use crate::shared::join_agent::PublicAgentData;

pub struct AgentStore {
    scope: Scope,
    broadcast: broadcast::Sender<AgentUpdate>,
}

#[derive(Clone, Debug)]
pub enum AgentUpdate {
    Add(SignedObject<PublicAgentData>),
}

impl AgentStore {
    pub fn open(scope: Scope) -> Arc<Self> {
        let (broadcast, _) = broadcast::channel(10);
        Arc::new(Self { scope, broadcast })
    }

    pub fn get_agent(&self, public_key: &[u8]) -> Result<Option<SignedObject<PublicAgentData>>> {
        let mut raw: Option<Vec<u8>> = None;
        self.scope.view(|b| {
            if let Some(data) = b.get_kv(public_key) {
                raw = Some(data.value().to_vec());
            }

            Ok(())
        })?;

        Ok(match raw {
            Some(bytes) => Some(SignedObject::<PublicAgentData>::from_bytes(bytes)?),
            None => None,
        })
    }

    pub fn add_agent(&self, agent: SignedObject<PublicAgentData>) -> Result<()> {
        self.scope.update(|b| {
            let key = agent.cert.public_key().to_owned();
            b.put(key, agent.to_bytes().to_owned())?;

            Ok(())
        })?;

        self.broadcast.send(AgentUpdate::Add(agent))?;

        Ok(())
    }

    pub fn list_agents(&self) -> Result<Vec<SignedObject<PublicAgentData>>> {
        let mut raw = Vec::<Vec<u8>>::new();
        self.scope.view(|b| {
            for v in b.cursor() {
                if let Data::KeyValue(v) = v {
                    raw.push(v.value().to_vec())
                }
            }

            Ok(())
        })?;

        let mut agents = Vec::<SignedObject<PublicAgentData>>::new();

        for v in raw {
            agents.push(SignedObject::from_bytes(v)?);
        }

        Ok(agents)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentUpdate> {
        self.broadcast.subscribe()
    }
}
