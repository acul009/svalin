use async_pty::{TerminalInput, TerminalSize};
use iced::{
    Task,
    widget::{center, stack, text},
};
use sipper::sipper;
use svalin::client::device::Device;

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
        let (term_display, terminal_emulator_task) = frozen_term::Terminal::new(25, 80);

        let (send, recv) = device.open_terminal(TerminalSize { rows: 25, cols: 80 });

        let read_task = Task::stream(sipper(move |mut sender| async move {
            let mut recv = recv;
            while let Some(output) = recv.recv().await {
                match output {
                    Ok(output) => sender.send(Message::Output(output)).await,
                    Err(_) => sender.send(Message::Unavailable).await,
                }
            }

            sender.send(Message::Closed).await;
        }));

        let task = Task::batch([terminal_emulator_task.map(Message::Terminal), read_task]);

        (
            Self {
                term_display,
                state: State::Pending,
                send,
            },
            task,
        )
    }

    #[must_use]
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Output(output) => {
                self.state = State::Ready;
                self.term_display.advance_bytes(&output);
                Task::none()
            }
            Message::Closed => {
                self.state = State::Closed;
                Task::none()
            }
            Message::Unavailable => {
                self.state = State::Unavailable;
                Task::none()
            }
            Message::Terminal(message) => {
                let action = self.term_display.update(message);
                match action {
                    frozen_term::Action::None => Task::none(),
                    frozen_term::Action::Run(task) => task.map(Message::Terminal),
                    frozen_term::Action::Resize(size) => {
                        let size = async_pty::TerminalSize {
                            cols: size.cols as u16,
                            rows: size.rows as u16,
                        };
                        let _ = self.send.try_send(TerminalInput::Resize(size));
                        Task::none()
                    }
                    frozen_term::Action::Input(input) => {
                        let _ = self.send.try_send(TerminalInput::Input(input));
                        Task::none()
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
