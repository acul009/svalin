use std::{collections::HashMap, sync::Arc};

use svalin_rpc::rpc::{
    command::CommandHandler,
    session::{Session, SessionOpen},
};
use tokio::{sync::Mutex, task::AbortHandle};

use self::{accept_handler::JoinAcceptHandler, request_handler::JoinRequestHandler};

pub mod accept_handler;
pub mod request_handler;

#[derive(Clone)]
pub struct ServerJoinManager {
    data: Arc<Mutex<ServerJoinManagerData>>,
}

struct ServerJoinManagerData {
    session_map: HashMap<String, (Session<SessionOpen>, AbortHandle)>,
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
        mut session: Session<SessionOpen>,
    ) -> Result<(), Session<SessionOpen>> {
        let mut data = self.data.lock().await;

        if data.session_map.contains_key(&join_code) {
            return Err(session);
        }

        session.write_object(&join_code).await.unwrap();

        let join_code_clone = join_code.clone();
        let data_clone = Arc::downgrade(&self.data);

        let abort_handle = data.joinset.spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(60 * 5)).await;
            if let Some(data) = data_clone.upgrade() {
                let mut data = data.lock().await;
                data.session_map.remove(&join_code_clone);
            }
        });

        data.session_map.insert(join_code, (session, abort_handle));

        Ok(())
    }

    pub async fn get_session(&self, join_code: &str) -> Option<Session<SessionOpen>> {
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
