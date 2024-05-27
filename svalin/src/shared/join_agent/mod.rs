use std::{collections::HashMap, sync::Arc};

use svalin_rpc::SessionOpen;
use tokio::{sync::Mutex, task::AbortHandle};

use self::{accept_handler::JoinAcceptHandler, request_handler::JoinRequestHandler};

mod accept_handler;
mod request_handler;

#[derive(Clone)]
pub struct ServerJoinManager {
    data: Arc<Mutex<ServerJoinManagerData>>,
}

struct ServerJoinManagerData {
    session_map: HashMap<String, (svalin_rpc::Session<SessionOpen>, AbortHandle)>,
    joinset: tokio::task::JoinSet<()>,
}

impl Drop for ServerJoinManagerData {
    fn drop(&mut self) {
        self.joinset.abort_all();
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

    pub async fn add_session(&self, joincode: String, session: svalin_rpc::Session<SessionOpen>) {
        let mut data = self.data.lock().await;

        let joincode_clone = joincode.clone();
        // let stopped = session.stopped();
        let data_clone = Arc::downgrade(&self.data);

        let abort_handle = data.joinset.spawn(async move {
            // stopped.await;
            if let Some(data) = data_clone.upgrade() {
                let mut data = data.lock().await;
                data.session_map.remove(&joincode_clone);
            }
        });

        data.session_map.insert(joincode, (session, abort_handle));
    }

    pub async fn get_session(
        &self,
        joincode: &str,
    ) -> Option<Arc<svalin_rpc::Session<SessionOpen>>> {
        // let mut data = self.data.lock().await;

        // if let Some((connection, abort_handle)) = data.session_map.remove(joincode) {
        //     abort_handle.abort();
        //     Some(connection)
        // } else {
        //     None
        // }
        todo!()
    }

    pub fn create_request_handler(&self) -> JoinRequestHandler {
        JoinRequestHandler::new(self.clone())
    }

    pub fn create_accept_handler(&self) -> JoinAcceptHandler {
        JoinAcceptHandler::new(self.clone())
    }
}
