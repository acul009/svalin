use std::collections::HashMap;

use iced::{
    Subscription, Task,
    widget::{center, text},
    window,
};
use svalin::client::device::Device;
use terminal::TerminalWindow;

use crate::Element;

pub mod terminal;

#[derive(Debug, Clone)]
pub enum Message {
    Forwarded {
        id: window::Id,
        message: WindowMessage,
    },
    WindowClosed(window::Id),
    None,
}

pub struct WindowHelper {
    windows: HashMap<window::Id, WindowContent>,
}

impl WindowHelper {
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
        }
    }

    pub fn add_terminal(&mut self, device: Device) -> Task<Message> {
        let (window_id, task1) = Self::new_window();

        let (terminal, task2) = TerminalWindow::start(device);

        self.windows
            .insert(window_id, WindowContent::Terminal(terminal));

        Task::batch(vec![
            task1,
            task2.map(move |message| Message::Forwarded {
                id: window_id,
                message: WindowMessage::Terminal(message),
            }),
        ])
    }

    fn new_window() -> (window::Id, Task<Message>) {
        let (id, task) = window::open(window::Settings {
            ..Default::default()
        });

        (id, task.discard())
    }

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
            Message::WindowClosed(id) => {
                self.windows.remove(&id);
                Task::none()
            }
            Message::None => Task::none(),
        }
    }

    pub fn title(&self, window_id: window::Id) -> String {
        if let Some(window) = self.windows.get(&window_id) {
            window.title()
        } else {
            "Window Error".to_string()
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        window::close_events().map(Message::WindowClosed)
    }
}

#[derive(Debug, Clone)]
pub enum WindowMessage {
    Terminal(terminal::Message),
}

pub enum WindowContent {
    Terminal(terminal::TerminalWindow),
    Todo,
}

impl WindowContent {
    fn view(&self) -> Element<WindowMessage> {
        match self {
            Self::Terminal(terminal) => terminal.view().map(WindowMessage::Terminal),
            Self::Todo => todo!(),
        }
    }

    fn update(&mut self, message: WindowMessage) -> Task<WindowMessage> {
        match message {
            WindowMessage::Terminal(message) => {
                if let Self::Terminal(terminal) = self {
                    terminal.update(message);
                }
                Task::none()
            }
        }
    }

    pub fn title(&self) -> String {
        match self {
            Self::Terminal(terminal) => terminal.title(),
            Self::Todo => "Todo".to_string(),
        }
    }
}
