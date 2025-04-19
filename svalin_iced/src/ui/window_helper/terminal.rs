use std::sync::Arc;

use iced::{
    Subscription, Task,
    advanced::subscription,
    widget::{center, text},
    window,
};
use svalin::{
    client::device::{Device, RemoteData},
    shared::commands::terminal::RemoteTerminal,
};

use crate::{Element, ui::widgets::loading};

#[derive(Debug, Clone)]
pub enum Message {
    Terminal(frozen_term::Message),
    Ready(Arc<RemoteTerminal>),
    Unavailable,
}

pub struct TerminalWindow {
    term_display: frozen_term::Terminal,
    remote: RemoteData<RemoteTerminal>,
    current_size: svalin_sysctl::pty::TerminalSize,
}

impl TerminalWindow {
    pub fn start(device: Device) -> (Self, Task<Message>) {
        let current_size = svalin_sysctl::pty::TerminalSize { cols: 80, rows: 25 };

        let (term_display, task1) =
            frozen_term::Terminal::new(current_size.rows, current_size.cols);

        let task2 = Task::future(async move {
            match device.open_terminal().await {
                Ok(remote) => Message::Ready(Arc::new(remote)),
                Err(_) => Message::Unavailable,
            }
        });

        let task = Task::batch(vec![task1.map(Message::Terminal), task2]);

        (
            Self {
                term_display,
                remote: RemoteData::Pending,
                current_size,
            },
            task,
        )
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Ready(remote) => {
                if let Ok(remote) = Arc::try_unwrap(remote) {
                    self.remote = RemoteData::Ready(remote);
                }
            }
            Message::Unavailable => {
                self.remote = RemoteData::Unavailable;
            }
            Message::Terminal(message) => {
                let action = self.term_display.update(message);
                match action {
                    frozen_term::Action::None => {}
                    frozen_term::Action::Resize(size) => {
                        let size = svalin_sysctl::pty::TerminalSize {
                            cols: size.cols as u16,
                            rows: size.rows as u16,
                        };
                        if let RemoteData::Ready(remote) = &self.remote {
                            remote.try_resize(size.clone());
                        }
                        self.current_size = size;
                    }
                    frozen_term::Action::Input(input) => {
                        if let RemoteData::Ready(remote) = &self.remote {
                            remote.try_write(input);
                        }
                    }
                }
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        match &self.remote {
            RemoteData::Pending => loading(t!("terminal.connecting")).into(),
            RemoteData::Unavailable => center(text(t!("terminal.unavailable"))).into(),
            RemoteData::Ready(_remote) => self.term_display.view().map(Message::Terminal),
        }
    }

    pub fn title(&self) -> String {
        self.term_display.get_title().to_string()
    }

    pub fn subscription(&self, id: window::Id) -> Subscription<Message> {
        
        match &self.remote {
            RemoteData::Pending => Subscription::none(),
            RemoteData::Unavailable => Subscription::none(),
            RemoteData::Ready(remote) => Subscription::run_with_id(id, stream),
        }
    }
}
