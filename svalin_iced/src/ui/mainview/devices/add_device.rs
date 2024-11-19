use std::{mem, sync::Arc};

use iced::{
    widget::{button, text, text_input},
    Task,
};
use svalin::client::{add_agent::WaitingForConfirmCode, Client};

use crate::{
    fl,
    ui::{
        screen::SubScreen,
        types::error_display_info::ErrorDisplayInfo,
        widgets::{form, loading},
    },
};

#[derive(Debug, Clone)]
pub enum Message {
    Input(Input),
    Exit,
    Error(ErrorDisplayInfo<Arc<anyhow::Error>>),
    Continue,
    Back,
    WaitingForDeviceName(Arc<WaitingForConfirmCode>),
    Success,
}

impl From<Message> for super::Message {
    fn from(value: Message) -> Self {
        match value {
            Message::Exit => Self::ShowList,
            msg => Self::AddDevice(msg),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Input {
    JoinCode(String),
    DeviceName(String),
    ConfirmCode(String),
}

impl Input {
    pub fn update(self, add_device: &mut AddDevice) {
        match self {
            Input::JoinCode(new_join_code) => {
                add_device.join_code = new_join_code;
            }
            Input::DeviceName(new_device_name) => {
                add_device.device_name = new_device_name;
            }
            Input::ConfirmCode(new_confirm_code) => {
                add_device.confirm_code = new_confirm_code;
            }
        }
    }
}

pub enum State {
    Error(ErrorDisplayInfo<Arc<anyhow::Error>>),
    Loading(String),
    JoinCode,
    ConfirmCode,
    DeviceName,
    Success,
}

pub struct AddDevice {
    client: Arc<Client>,
    state: State,
    join_code: String,
    confirm_code: String,
    device_name: String,
    waiting: Option<WaitingForConfirmCode>,
}

impl AddDevice {
    pub fn start(client: Arc<Client>) -> (Self, Task<Message>) {
        (
            Self {
                client,
                state: State::JoinCode,
                join_code: String::new(),
                confirm_code: String::new(),
                device_name: String::new(),
                waiting: None,
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
            Message::Success => {
                self.state = State::Success;
                Task::none()
            }
            Message::Continue => match &mut self.state {
                State::Error(_) => Task::none(),
                State::JoinCode => {
                    let join_code = self.join_code.clone();
                    let client = self.client.clone();
                    self.state = State::Loading(fl!("connect-to-device"));
                    Task::future(async move {
                        let waiting = client.add_agent_with_code(join_code.clone()).await;

                        match waiting {
                            Err(err) => Message::Error(ErrorDisplayInfo::new(
                                Arc::new(err),
                                fl!("join-code-error"),
                            )),
                            Ok(waiting) => Message::WaitingForDeviceName(Arc::new(waiting)),
                        }
                    })
                }
                State::DeviceName => {
                    self.state = State::ConfirmCode;
                    text_input::focus("confirm-code")
                }
                State::ConfirmCode => match self.waiting.take() {
                    None => {
                        self.state = State::Error(ErrorDisplayInfo::new(
                            Arc::new(anyhow::anyhow!("waiting for device name")),
                            fl!("join-code-error"),
                        ));
                        Task::none()
                    }
                    Some(waiting) => {
                        let confirm_code = self.confirm_code.clone();
                        let device_name = self.device_name.clone();
                        self.state = State::Loading(fl!("enrolling-device"));
                        Task::future(async move {
                            let joined = waiting.confirm(confirm_code, device_name).await;

                            match joined {
                                Err(err) => Message::Error(ErrorDisplayInfo::new(
                                    Arc::new(err),
                                    fl!("confirm-code-error"),
                                )),
                                Ok(_) => Message::Success,
                            }
                        })
                    }
                },
                State::Success | State::Loading(_) => Task::none(),
            },
            Message::Back => match &mut self.state {
                State::JoinCode => Task::done(Message::Exit),
                State::Error(_) | State::DeviceName => {
                    self.state = State::JoinCode;
                    self.join_code = String::new();
                    self.confirm_code = String::new();
                    self.waiting = None;
                    text_input::focus("join-code")
                }
                State::ConfirmCode => {
                    self.state = State::DeviceName;
                    self.confirm_code = String::new();
                    text_input::focus("device-name")
                }
                State::Success | State::Loading(_) => Task::none(),
            },
            Message::WaitingForDeviceName(waiting) => {
                let waiting = Arc::into_inner(waiting).unwrap();

                self.waiting = Some(waiting);
                self.state = State::DeviceName;

                text_input::focus("device-name")
            }
            Message::Exit => unreachable!(),
        }
    }

    fn view(&self) -> crate::Element<Self::Message> {
        match &self.state {
            State::Error(error) => error.view().on_close(Message::Back).into(),
            State::Loading(message) => loading(message).into(),
            State::JoinCode => form()
                .title(fl!("input-join-code"))
                .control(
                    text_input(&fl!("join-code"), &self.join_code)
                        .id("join-code")
                        .on_input(|input| Message::Input(Input::JoinCode(input)))
                        .on_submit(Message::Continue),
                )
                .primary_action(button(text(fl!("continue"))).on_press(Message::Continue))
                .secondary_action(button(text(fl!("back"))).on_press(Message::Back))
                .into(),
            State::DeviceName => form()
                .title(fl!("input-device-name"))
                .control(
                    text_input(&fl!("device-name"), &self.device_name)
                        .id("device-name")
                        .on_input(|input| Message::Input(Input::DeviceName(input)))
                        .on_submit(Message::Continue),
                )
                .primary_action(button(text(fl!("continue"))).on_press(Message::Continue))
                .secondary_action(button(text(fl!("back"))).on_press(Message::Back))
                .into(),
            State::ConfirmCode => form()
                .title(fl!("input-confirm-code"))
                .control(
                    text_input(&fl!("confirm-code"), &self.confirm_code)
                        .id("confirm-code")
                        .on_input(|input| Message::Input(Input::ConfirmCode(input)))
                        .on_submit(Message::Continue),
                )
                .primary_action(button(text(fl!("continue"))).on_press(Message::Continue))
                .secondary_action(button(text(fl!("back"))).on_press(Message::Back))
                .into(),
            State::Success => form()
                .title(fl!("success"))
                .control(text(fl!("success-join")))
                .primary_action(button(text(fl!("back"))).on_press(Message::Exit))
                .into(),
        }
    }
}
