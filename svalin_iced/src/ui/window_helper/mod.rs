use std::collections::{BTreeMap, HashMap};

use iced::{
    Task,
    widget::{center, text},
    window,
};
use svalin::shared::commands::terminal::RemoteTerminal;
use terminal::TerminalWindow;

use crate::Element;

pub mod terminal;

#[derive(Debug, Clone)]
pub enum Message {
    Forwarded {
        id: window::Id,
        message: WindowMessage,
    },
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

    pub fn add_terminal(&mut self, terminal: RemoteTerminal) -> Task<Message> {
        let (id, task) = Self::new_window();
        
        let terminal = TerminalWindow::
        
    }

    fn new_window() -> (window::Id, Task<Message>) {
        let (id, task) = window::open(window::Settings {
            ..Default::default()
        });

        (id, task.map(|_| Message::None))
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
        }
    }

    pub fn title(&self, window_id: window::Id) -> String {
        if let Some(window) = self.windows.get(&window_id) {
            window.title()
        } else {
            String::from("Window Error")
        }
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
            Self::Todo => String::from("Todo"),
        }
    }
}
