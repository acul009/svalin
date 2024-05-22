use std::{collections::HashMap, sync::Arc};

use tokio::{sync::Mutex, task::AbortHandle};

use self::{accept_handler::JoinAcceptHandler, request_handler::JoinRequestHandler};

mod accept_handler;
mod request_handler;

#[derive(Clone)]
pub struct ServerJoinManager {
    data: Arc<Mutex<ServerJoinManagerData>>,
}

struct ServerJoinManagerData {
    connection_map: HashMap<String, (Arc<dyn svalin_rpc::Connection>, AbortHandle)>,
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
            connection_map: HashMap::new(),
            joinset: tokio::task::JoinSet::new(),
        };

        Self {
            data: Arc::new(Mutex::new(data)),
        }
    }

    pub async fn add_connection(
        &self,
        joincode: String,
        connection: Arc<dyn svalin_rpc::Connection>,
    ) {
        let mut data = self.data.lock().await;

        let joincode_clone = joincode.clone();
        let connection_clone = connection.clone();
        let data_clone = Arc::downgrade(&self.data);

        let abort_handle = data.joinset.spawn(async move {
            connection_clone.closed().await;
            if let Some(data) = data_clone.upgrade() {
                let mut data = data.lock().await;
                data.connection_map.remove(&joincode_clone);
            }
        });

        data.connection_map
            .insert(joincode, (connection, abort_handle));
    }

    pub async fn get_connection(&self, joincode: &str) -> Option<Arc<dyn svalin_rpc::Connection>> {
        let mut data = self.data.lock().await;

        if let Some((connection, abort_handle)) = data.connection_map.remove(joincode) {
            abort_handle.abort();
            Some(connection)
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
