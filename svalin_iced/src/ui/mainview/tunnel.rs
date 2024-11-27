use iced::widget::text;

use crate::ui::screen::SubScreen;

#[derive(Debug, Clone)]
pub enum Message {}

impl From<Message> for super::Message {
    fn from(value: Message) -> Self {
        Self::Tunnel(value)
    }
}

pub struct TunnelUi {}

impl TunnelUi {
    pub fn new() -> Self {
        Self {}
    }
}

impl SubScreen for TunnelUi {
    type Message = Message;

    fn update(&mut self, message: Self::Message) -> iced::Task<Self::Message> {
        match message {}
    }

    fn view(&self) -> crate::Element<Self::Message> {
        text("Tunnel").into()
    }
}
