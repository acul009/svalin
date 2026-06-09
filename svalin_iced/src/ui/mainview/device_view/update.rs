use iced::widget::{button, column, text_input};

use crate::{Element, ui::widgets::card};

#[derive(Debug, Clone)]
pub enum Message {
    ChangeCustomUrl(String),
    StartAgentUpdate,
}

pub enum Action {
    StartAgentUpdate(String),
    None,
}

pub struct State {
    custom_url: String,
}

impl State {
    pub fn new() -> Self {
        Self {
            custom_url: String::new(),
        }
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::ChangeCustomUrl(custom_url) => {
                self.custom_url = custom_url;
                Action::None
            }
            Message::StartAgentUpdate => Action::StartAgentUpdate(self.custom_url.clone()),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        card(
            column![
                text_input("Update URL", &self.custom_url).on_input(Message::ChangeCustomUrl),
                button("Update").on_press_maybe(if self.custom_url.is_empty() {
                    None
                } else {
                    Some(Message::StartAgentUpdate)
                },)
            ]
            .spacing(10),
        )
        .into()
    }
}
