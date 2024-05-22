use std::{collections::HashMap, sync::Arc};

use tokio::sync::Mutex;

struct ServerJoinWrapper {
    manager: Arc<ServerJoinManager>,
}

impl ServerJoinWrapper {
    pub fn new() -> Self {
        let manager = Arc::new(ServerJoinManager::new());
        let manager_clone = manager.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                manager_clone.cleanup().await;
            }
        });

        Self { manager }
    }
}

struct ServerJoinManager {
    connection_map: Mutex<HashMap<String, Box<dyn svalin_rpc::Connection>>>,
    joinset: tokio::task::JoinSet<()>,
    to_remove: Mutex<Vec<String>>,
}

impl ServerJoinManager {
    pub fn new() -> Self {
        let manager = Self {
            connection_map: Mutex::new(HashMap::new()),
            to_remove: Mutex::new(vec![]),
            joinset: tokio::task::JoinSet::new(),
        };

        manager
    }

    pub async fn cleanup(self: &Arc<Self>) {
        let mut connection_map = self.connection_map.lock().await;

        let mut to_remove = self.to_remove.lock().await;
        to_remove.clear();

        connection_map.iter().for_each(|(joincode, connection)| {
            if connection.is_closed() {
                to_remove.push(joincode.clone());
            }
        });

        to_remove.iter().for_each(|joincode| {
            connection_map.remove(joincode);
        });
    }

    pub async fn add_connection(
        self: Arc<Self>,
        joincode: String,
        connection: Box<dyn svalin_rpc::Connection>,
    ) {
        let mut connection_map = self.connection_map.lock().await;

        connection_map.insert(joincode, connection);
    }
}

struct JoinRequestHandler {
    manager: Arc<ServerJoinManager>,
}

struct JoinAcceptHandler {
    manager: Arc<ServerJoinManager>,
}
