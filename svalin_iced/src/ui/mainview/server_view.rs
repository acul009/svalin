use iced::{
    Widget,
    widget::{column, text},
};

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

    pub fn update(&mut self, message: Message) {
        match message {
            Message::SelectScreen(screen) => self.screen = screen,
            Message::Sessions(message) => self.sessions.update(message),
        }
    }

    pub fn view(&self) -> impl Widget<Message> + '_ {
        match self.screen {
            Screen::Sessions => sessions_view::view(&self.sessions)
                .map(Message::Sessions)
                .boxed(),
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
