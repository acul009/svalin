use std::{collections::HashMap, sync::Arc};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio::sync::{RwLock, RwLockWriteGuard};

use crate::rpc::session::Session;

#[async_trait]
pub trait CommandHandler: Sync + Send {
    fn key(&self) -> String;
    async fn handle(&self, session: &mut Session) -> Result<()>;
}

#[async_trait]
pub trait TakeableCommandHandler: Sync + Send {
    fn key(&self) -> String;
    async fn handle(&self, session: &mut Option<Session>) -> Result<()>;
}

#[async_trait]
impl<T> TakeableCommandHandler for T
where
    T: CommandHandler,
{
    fn key(&self) -> String {
        self.key()
    }

    async fn handle(&self, session: &mut Option<Session>) -> Result<()> {
        if let Some(session) = session {
            self.handle(session).await
        } else {
            Err(anyhow!("tried executing commandhandler with None"))
        }
    }
}

#[derive(Clone)]
pub struct HandlerCollection {
    commands: Arc<RwLock<HashMap<String, Arc<dyn TakeableCommandHandler>>>>,
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

    pub async fn get(&self, key: &str) -> Option<Arc<dyn TakeableCommandHandler>> {
        let commands = self.commands.read().await;
        commands.get(key).cloned()
    }
}

pub struct ChainCommandAdder<'a> {
    lock: RwLockWriteGuard<'a, HashMap<String, Arc<dyn TakeableCommandHandler>>>,
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
