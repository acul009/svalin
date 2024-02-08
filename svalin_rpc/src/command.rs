use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;

use crate::session::{Session, SessionOpen};

#[async_trait]
pub trait CommandHandler {
    fn key(&self) -> String;
    async fn handle(&mut self, mut session: Session<SessionOpen>) -> Result<()>;
}

pub struct HandlerCollection {
    commands: HashMap<String, Box<dyn CommandHandler>>,
}

impl HandlerCollection {
    pub fn get(&self, key: &str) -> Option<&mut Box<dyn CommandHandler>> {
        self.commands.get_mut(key)
    }
}
