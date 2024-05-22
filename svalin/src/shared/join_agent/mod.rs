use std::{collections::HashMap, sync::Arc};

use tokio::sync::Mutex;

#[derive(Clone)]
struct ServerJoinManager {
    data: Arc<Mutex<ServerJoinManagerData>>,
}

struct ServerJoinManagerData {
    connection_map: HashMap<String, Arc<dyn svalin_rpc::Connection>>,
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
        &mut self,
        joincode: String,
        connection: Arc<dyn svalin_rpc::Connection>,
    ) {
        let mut data = self.data.lock().await;

        let joincode_clone = joincode.clone();
        let connection_clone = connection.clone();

        data.connection_map.insert(joincode, connection);

        let data_clone = Arc::downgrade(&self.data);

        data.joinset.spawn(async move {
            connection_clone.closed().await;
            if let Some(data) = data_clone.upgrade() {
                let mut data = data.lock().await;
                data.connection_map.remove(&joincode_clone);
            }
        });
    }

    pub fn create_request_handler(&self) -> JoinRequestHandler {
        JoinRequestHandler {
            manager: self.clone(),
        }
    }

    pub fn create_accept_handler(&self) -> JoinAcceptHandler {
        JoinAcceptHandler {
            manager: self.clone(),
        }
    }
}

// TODO
struct JoinRequestHandler {
    manager: ServerJoinManager,
}

//TODO
struct JoinAcceptHandler {
    manager: ServerJoinManager,
}
