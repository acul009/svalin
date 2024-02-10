use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::session::{Session, SessionOpen};

#[async_trait]
pub trait CommandHandler: Sync + Send {
    fn key(&self) -> String;
    async fn handle(&self, mut session: Session<SessionOpen>) -> Result<()>;
}

pub struct HandlerCollection {
    commands: RwLock<HashMap<String, Arc<dyn CommandHandler>>>,
}

impl HandlerCollection {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            commands: RwLock::new(HashMap::new()),
        })
    }

    pub async fn add<'a, T>(self: &'a Arc<Self>, command: T) -> &'a Arc<Self>
    where
        T: CommandHandler + 'static,
    {
        let mut commands = self.commands.write().await;
        commands.insert(command.key(), Arc::new(command));
        self
    }

    pub async fn get(&self, key: &str) -> Option<Arc<dyn CommandHandler>> {
        let commands = self.commands.read().await;
        commands.get(key).cloned()
    }
}
