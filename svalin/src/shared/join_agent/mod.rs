use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use svalin_pki::{Certificate, PermCredentials};
use svalin_rpc::rpc::session::{Session};
use tokio::{sync::Mutex, task::AbortHandle};
use tracing::field::debug;

use self::{accept_handler::JoinAcceptHandler, request_handler::JoinRequestHandler};

pub mod accept_handler;
pub mod add_agent;
pub mod request_handler;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PublicAgentData {
    pub name: String,
    pub cert: Certificate,
}

#[derive(Debug)]
pub struct AgentInitPayload {
    pub address: String,
    pub credentials: PermCredentials,
    pub root: Certificate,
    pub upstream: Certificate,
}

#[derive(Clone)]
pub struct ServerJoinManager {
    data: Arc<Mutex<ServerJoinManagerData>>,
}

struct ServerJoinManagerData {
    session_map: HashMap<String, (Session, AbortHandle)>,
    joinset: tokio::task::JoinSet<()>,
}

impl Drop for ServerJoinManagerData {
    fn drop(&mut self) {
        self.joinset.abort_all();
    }
}

impl Default for ServerJoinManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerJoinManager {
    pub fn new() -> Self {
        let data = ServerJoinManagerData {
            session_map: HashMap::new(),
            joinset: tokio::task::JoinSet::new(),
        };

        Self {
            data: Arc::new(Mutex::new(data)),
        }
    }

    pub async fn add_session(
        &self,
        join_code: String,
        mut session: Session,
    ) -> Result<(), Session> {
        let mut data = self.data.lock().await;

        if data.session_map.contains_key(&join_code) {
            return Err(session);
        }

        session.write_object(&join_code).await.unwrap();

        let join_code_clone = join_code.clone();
        let data_clone = Arc::downgrade(&self.data);

        let abort_handle = data.joinset.spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(60 * 5)).await;
            debug("timeout for agent join request on server reached");
            if let Some(data) = data_clone.upgrade() {
                let mut data = data.lock().await;
                data.session_map.remove(&join_code_clone);
            }
        });

        data.session_map.insert(join_code, (session, abort_handle));

        Ok(())
    }

    pub async fn get_session(&self, join_code: &str) -> Option<Session> {
        let mut data = self.data.lock().await;

        if let Some((session, abort_handle)) = data.session_map.remove(join_code) {
            abort_handle.abort();
            Some(session)
        } else {
            None
        }
    }

    pub fn create_request_handler(&self) -> JoinRequestHandler {
        JoinRequestHandler::new(self.clone())
    }

    pub fn create_accept_handler(&self) -> JoinAcceptHandler {
        JoinAcceptHandler::new(self.clone())
    }
}

async fn derive_confirm_code(
    params: svalin_pki::ArgonParams,
    derived_secret: &[u8; 32],
) -> Result<String> {
    let hashed = params.derive_key(derived_secret.to_vec()).await?;

    let number = u64::from_be_bytes(hashed[0..8].try_into().unwrap());

    Ok((number % 1000000).to_string())
}
