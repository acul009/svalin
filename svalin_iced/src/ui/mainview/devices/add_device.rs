use std::sync::Arc;

use iced::{
    widget::{button, text, text_input},
    Task,
};
use svalin::client::{add_agent::WaitingForConfirmCode, Client};

use crate::{
    fl,
    ui::{screen::SubScreen, types::error_display_info::ErrorDisplayInfo, widgets::form},
};

#[derive(Debug, Clone)]
pub enum Message {
    Exit,
    Error(ErrorDisplayInfo<Arc<anyhow::Error>>),
    Continue,
    Back,
    WaitingForConfirmCode(Arc<WaitingForConfirmCode>),
    Input(Input),
}

impl From<Message> for super::Message {
    fn from(value: Message) -> Self {
        match value {
            Message::Exit => Self::Reset,
            msg => Self::AddDevice(msg),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Input {
    JoinCode(String),
    ConfirmCode(String),
}

impl Input {
    pub fn update(self, add_device: &mut AddDevice) {
        match self {
            Input::JoinCode(new_join_code) => {
                if let State::JoinCode { join_code } = &mut add_device.state {
                    *join_code = new_join_code;
                }
            }
            Input::ConfirmCode(new_confirm_code) => {
                if let State::ConfirmCode { confirm_code, .. } = &mut add_device.state {
                    *confirm_code = new_confirm_code;
                }
            }
        }
    }
}

pub enum State {
    Error(ErrorDisplayInfo<Arc<anyhow::Error>>),
    JoinCode {
        join_code: String,
    },
    ConfirmCode {
        waiting: WaitingForConfirmCode,
        confirm_code: String,
    },
}

pub struct AddDevice {
    client: Arc<Client>,
    state: State,
}

impl AddDevice {
    pub fn start(client: Arc<Client>) -> (Self, Task<Message>) {
        (
            Self {
                client,
                state: State::JoinCode {
                    join_code: String::new(),
                },
            },
            text_input::focus("join-code"),
        )
    }
}

impl SubScreen for AddDevice {
    type Message = Message;

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::Error(error) => {
                self.state = State::Error(error);
                Task::none()
            }
            Message::Input(input) => {
                input.update(self);

                Task::none()
            }
            Message::Continue => match &mut self.state {
                State::Error(_) => Task::none(),
                State::JoinCode { join_code, .. } => {
                    let join_code = join_code.clone();
                    let client = self.client.clone();
                    Task::future(async move {
                        let waiting = client.add_agent_with_code(join_code.clone()).await;

                        match waiting {
                            Err(err) => Message::Error(ErrorDisplayInfo::new(
                                Arc::new(err),
                                fl!("join-code-error"),
                            )),
                            Ok(waiting) => Message::WaitingForConfirmCode(Arc::new(waiting)),
                        }
                    })
                }
                State::ConfirmCode {
                    confirm_code,
                    waiting,
                } => Task::future(async move { 
                    let joined = waiting.confirm(confirm_code, agent_name)
                 }),
            },
            Message::Back => match &mut self.state {
                State::Error(_) => {
                    self.state = State::JoinCode {
                        join_code: String::new(),
                    };
                    text_input::focus("join-code")
                }
                State::JoinCode { .. } => Task::done(Message::Exit),
                State::ConfirmCode { .. } => {
                    self.state = State::JoinCode {
                        join_code: String::new(),
                    };
                    Task::none()
                }
            },
            Message::WaitingForConfirmCode(waiting) => {
                let waiting = Arc::into_inner(waiting).unwrap();

                self.state = State::ConfirmCode {
                    confirm_code: String::new(),
                    waiting,
                };

                Task::none()
            }
            Message::Exit => unreachable!(),
        }
    }

    fn view(&self) -> crate::Element<Self::Message> {
        match &self.state {
            State::Error(error) => error.view().on_close(Message::Back).into(),
            State::JoinCode { join_code } => form()
                .title(fl!("input-join-code"))
                .control(
                    text_input(&fl!("join-code"), join_code)
                        .id("join-code")
                        .on_input(|input| Message::Input(Input::JoinCode(input))),
                )
                .primary_action(button(text(fl!("continue"))).on_press(Message::Continue))
                .secondary_action(button(text(fl!("back"))).on_press(Message::Back))
                .into(),
            State::ConfirmCode { confirm_code, .. } => form()
                .title(fl!("input-confirm-code"))
                .control(
                    text_input(&fl!("confirm-code"), &confirm_code)
                        .id("confirm-code")
                        .on_input(|input| Message::Input(Input::ConfirmCode(input))),
                )
                .primary_action(button(text(fl!("continue"))).on_press(Message::Continue))
                .secondary_action(button(text(fl!("back"))).on_press(Message::Back))
                .into(),
        }
    }
}
