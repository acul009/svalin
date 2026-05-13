use iced::{
    Task,
    widget::{center, column, row, text},
};
use svalin::client::state::ClientState;
use svalin_pki::SpkiHash;

use crate::Element;

#[derive(Debug, Clone)]
pub enum Message {}

pub enum Action {
    None,
    Back,
    Run(Task<Message>),
}

pub struct DeviceView {
    spki_hash: SpkiHash,
}

impl DeviceView {
    pub fn new(spki_hash: SpkiHash) -> Self {
        Self { spki_hash }
    }

    pub fn update(&mut self, message: Message) -> Action {
        Action::None
    }

    pub fn view<'a>(&'a self, client_state: &'a ClientState) -> Element<'a, Message> {
        let Some(persistent) = client_state.persistent().get(&self.spki_hash) else {
            return center("Device not yet available").into();
        };

        column![row!["Name: ", text(persistent.name())]].into()
    }
}
