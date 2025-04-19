use iced::{
    Task,
    widget::{center, stack, text},
};
use sipper::sipper;
use svalin::{client::device::Device, shared::commands::terminal::TerminalInput};
use tokio::sync::mpsc;

use crate::{Element, ui::widgets::loading};

#[derive(Debug, Clone)]
pub enum Message {
    Terminal(frozen_term::Message),
    Output(Vec<u8>),
    Closed,
    Unavailable,
}

enum State {
    Pending,
    Ready,
    Unavailable,
    Closed,
}

pub struct TerminalWindow {
    term_display: frozen_term::Terminal,
    state: State,
    send: mpsc::Sender<TerminalInput>,
}

impl TerminalWindow {
    pub fn start(device: Device) -> (Self, Task<Message>) {
        let (term_display, task1) = frozen_term::Terminal::new(25, 80);

        let (send, recv) = device.open_terminal();

        let task2 = Task::stream(sipper(move |mut sender| async move {
            let mut recv = recv;
            while let Some(output) = recv.recv().await {
                match output {
                    Ok(output) => sender.send(Message::Output(output)).await,
                    Err(_) => sender.send(Message::Unavailable).await,
                }
            }

            Message::Closed
        }));

        let task = Task::batch(vec![task1.map(Message::Terminal), task2]);

        (
            Self {
                term_display,
                state: State::Ready,
                send,
            },
            task,
        )
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Output(output) => {
                self.state = State::Ready;
                self.term_display.advance_bytes(&output);
            }
            Message::Closed => {
                self.state = State::Closed;
            }
            Message::Unavailable => {
                self.state = State::Unavailable;
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
                        let _ = self.send.try_send(TerminalInput::Resize(size));
                    }
                    frozen_term::Action::Input(input) => {
                        let _ = self.send.try_send(TerminalInput::Input(input));
                    }
                }
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        match &self.state {
            State::Pending => loading(t!("terminal.connecting")).into(),
            State::Unavailable => center(text(t!("terminal.unavailable"))).into(),
            State::Ready => self.term_display.view().map(Message::Terminal),
            State::Closed => stack![
                self.term_display.view().map(Message::Terminal),
                center(text(t!("terminal.closed")))
            ]
            .into(),
        }
    }

    pub fn title(&self) -> String {
        self.term_display.get_title().to_string()
    }
}
