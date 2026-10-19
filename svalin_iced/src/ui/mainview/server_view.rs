use std::sync::Arc;

use iced::{
    Task, Widget,
    widget::{column, text},
};
use svalin::client::Client;

use crate::ui::widgets::button_list;

mod sessions_view;

#[derive(Debug, Clone)]
pub enum Message {
    SelectScreen(Screen),
    Sessions(sessions_view::Message),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Sessions,
}

pub struct ServerView {
    screen: Screen,
    sessions: sessions_view::SessionsView,
}

impl ServerView {
    pub fn new() -> Self {
        Self {
            screen: Screen::Sessions,
            sessions: sessions_view::SessionsView::new(),
        }
    }

    pub fn update(&mut self, message: Message, client: &Arc<Client>) -> Task<Message> {
        match message {
            Message::SelectScreen(screen) => {
                self.screen = screen;
                self.sessions
                    .update(sessions_view::Message::Refresh, client)
                    .map(Message::Sessions)
            }
            Message::Sessions(message) => {
                self.sessions.update(message, client).map(Message::Sessions)
            }
        }
    }

    pub fn view(&self) -> impl Widget<Message> + '_ {
        match self.screen {
            Screen::Sessions => self.sessions.view().map(Message::Sessions).boxed(),
        }
    }

    pub fn sidebar(&self) -> crate::Element<'_, Message> {
        column![
            button_list::entry(text(t!("server.sessions")))
                .on_press(Message::SelectScreen(Screen::Sessions))
                .into_widget(40, [8, 16], false)
        ]
        .boxed()
    }
}
