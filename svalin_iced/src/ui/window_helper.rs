use std::collections::HashMap;

use async_pty::TerminalInput;
use iced::{
    Task,
    widget::{center, text},
    window,
};
use terminal::TerminalWindow;
use tokio::sync::mpsc;

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

    pub fn add_terminal(
        &mut self,
        send: mpsc::Sender<TerminalInput>,
        recv: mpsc::Receiver<Vec<u8>>,
    ) -> Task<Message> {
        let (window_id, task1) = Self::new_window();

        let (terminal, task2) = TerminalWindow::start(send, recv);

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

    pub fn view(&self, id: window::Id) -> Element<'_, Message> {
        if let Some(window) = self.windows.get(&id) {
            window
                .view()
                .map(move |message| Message::Forwarded { id, message })
        } else {
            center(text(t!("window.error"))).into()
        }
    }

    #[must_use]
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
            Message::None => Task::none(),
        }
    }

    pub fn title(&self, window_id: window::Id) -> String {
        if let Some(window) = self.windows.get(&window_id) {
            window.title()
        } else {
            t!("window.error").to_string()
        }
    }

    pub(crate) fn close_window(&mut self, id: &window::Id) {
        self.windows.remove(id);
    }
}

#[derive(Debug, Clone)]
pub enum WindowMessage {
    Terminal(terminal::Message),
}

pub enum WindowContent {
    Terminal(terminal::TerminalWindow),
}

impl WindowContent {
    fn view(&self) -> Element<'_, WindowMessage> {
        match self {
            Self::Terminal(terminal) => terminal.view().map(WindowMessage::Terminal),
        }
    }

    fn update(&mut self, message: WindowMessage) -> Task<WindowMessage> {
        match message {
            WindowMessage::Terminal(message) => match self {
                WindowContent::Terminal(terminal) => {
                    terminal.update(message).map(WindowMessage::Terminal)
                }
            },
        }
    }

    pub fn title(&self) -> String {
        match self {
            Self::Terminal(terminal) => terminal.title(),
        }
    }
}
