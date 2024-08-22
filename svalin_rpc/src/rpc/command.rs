use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::{RwLock, RwLockWriteGuard};

use crate::rpc::session::{Session};

#[async_trait]
pub trait CommandHandler: Sync + Send {
    fn key(&self) -> String;
    async fn handle(&self, session: &mut Session) -> Result<()>;
}

#[derive(Clone)]
pub struct HandlerCollection {
    commands: Arc<RwLock<HashMap<String, Arc<dyn CommandHandler>>>>,
}

impl HandlerCollection {
    pub fn new() -> Self {
        Self {
            commands: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn chain(&self) -> ChainCommandAdder {
        let lock = self.commands.write().await;
        ChainCommandAdder { lock }
    }

    pub async fn get(&self, key: &str) -> Option<Arc<dyn CommandHandler>> {
        let commands = self.commands.read().await;
        commands.get(key).cloned()
    }
}

pub struct ChainCommandAdder<'a> {
    lock: RwLockWriteGuard<'a, HashMap<String, Arc<dyn CommandHandler>>>,
}

impl<'a> ChainCommandAdder<'a> {
    pub fn add<T>(&mut self, command: T) -> &mut Self
    where
        T: CommandHandler + 'static,
    {
        self.lock.insert(command.key(), Arc::new(command));
        self
    }
}
