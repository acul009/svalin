use std::collections::BTreeMap;

use iced::{
    Task,
    widget::{center, text},
    window,
};

use crate::Element;

pub enum Message {
    Forwarded {
        id: window::Id,
        message: WrappedMessage,
    },
}

pub struct WindowHelper {
    windows: BTreeMap<window::Id, WindowContent>,
}

impl WindowHelper {
    pub fn view(&self, id: window::Id) -> Element<Message> {
        if let Some(window) = self.windows.get(&id) {
            window
                .view()
                .map(move |message| Message::Forwarded { id, message })
        } else {
            center(text!("Window Error")).into()
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Forwarded { id, message } => {
                if let Some(window) = self.windows.get_mut(&id) {
                    window
                        .update(message)
                        .map(move |wrapped| Message::Forwarded {
                            id,
                            message: wrapped,
                        })
                } else {
                    Task::none()
                }
            }
        }
    }
}

enum WrappedMessage {}

enum WindowContent {}

impl WindowContent {
    pub fn view(&self) -> Element<WrappedMessage> {
        match self {
            _ => todo!(),
        }
    }

    pub fn update(&mut self, message: WrappedMessage) -> Task<WrappedMessage> {
        match self {
            _ => todo!(),
        }
    }
}
