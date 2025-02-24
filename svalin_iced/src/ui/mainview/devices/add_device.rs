use std::sync::Arc;

use iced::{
    Task,
    widget::{button, text, text_input},
};
use svalin::client::{Client, add_agent::WaitingForConfirmCode};

use crate::ui::{
    action::Action,
    screen::SubScreen,
    types::error_display_info::ErrorDisplayInfo,
    widgets::{form, loading},
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

pub enum Instruction {
    Exit,
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
    type Instruction = Instruction;
    type Message = Message;

    fn update(&mut self, message: Self::Message) -> Action<Instruction, Message> {
        match message {
            Message::Error(error) => {
                self.state = State::Error(error);
                Action::none()
            }
            Message::Input(input) => {
                input.update(self);
                Action::none()
            }
            Message::Success => {
                self.state = State::Success;
                Action::none()
            }
            Message::Continue => match &mut self.state {
                State::Error(_) => Action::none(),
                State::JoinCode => {
                    let join_code = self.join_code.clone();
                    let client = self.client.clone();
                    self.state = State::Loading(t!("add-device.connecting").to_string());
                    Task::future(async move {
                        let waiting = client.add_agent_with_code(join_code.clone()).await;

                        match waiting {
                            Err(err) => Message::Error(ErrorDisplayInfo::new(
                                Arc::new(err),
                                t!("add-device.error.join-code"),
                            )),
                            Ok(waiting) => Message::WaitingForDeviceName(Arc::new(waiting)),
                        }
                    })
                    .into()
                }
                State::DeviceName => {
                    self.state = State::ConfirmCode;
                    text_input::focus("confirm-code").into()
                }
                State::ConfirmCode => match self.waiting.take() {
                    None => {
                        self.state = State::Error(ErrorDisplayInfo::new(
                            Arc::new(anyhow::anyhow!("waiting for device name")),
                            t!("add-device.error.join-code"),
                        ));
                        Action::none()
                    }
                    Some(waiting) => {
                        let confirm_code = self.confirm_code.clone();
                        let device_name = self.device_name.clone();
                        self.state = State::Loading(t!("add-device.enrolling").to_string());
                        Task::future(async move {
                            let joined = waiting.confirm(confirm_code, device_name).await;

                            match joined {
                                Err(err) => Message::Error(ErrorDisplayInfo::new(
                                    Arc::new(err),
                                    t!("add-device.error.join-code"),
                                )),
                                Ok(_) => Message::Success,
                            }
                        })
                        .into()
                    }
                },
                State::Success | State::Loading(_) => Action::none(),
            },
            Message::Back => match &mut self.state {
                State::JoinCode => Action::instruction(Instruction::Exit),
                State::Error(_) | State::DeviceName => {
                    self.state = State::JoinCode;
                    self.join_code = String::new();
                    self.confirm_code = String::new();
                    self.waiting = None;
                    text_input::focus("join-code").into()
                }
                State::ConfirmCode => {
                    self.state = State::DeviceName;
                    self.confirm_code = String::new();
                    text_input::focus("device-name").into()
                }
                State::Success | State::Loading(_) => Action::none(),
            },
            Message::WaitingForDeviceName(waiting) => {
                let waiting = Arc::into_inner(waiting).unwrap();

                self.waiting = Some(waiting);
                self.state = State::DeviceName;

                text_input::focus("device-name").into()
            }
            Message::Exit => Action::instruction(Instruction::Exit),
        }
    }

    fn view(&self) -> crate::Element<Self::Message> {
        match &self.state {
            State::Error(error) => error.view().on_close(Message::Back).into(),
            State::Loading(message) => loading(message).into(),
            State::JoinCode => form()
                .title(t!("add-device.title.join-code"))
                .control(
                    text_input(&t!("add-device.input.join-code"), &self.join_code)
                        .id("join-code")
                        .on_input(|input| Message::Input(Input::JoinCode(input)))
                        .on_submit(Message::Continue),
                )
                .primary_action(button(text(t!("generic.continue"))).on_press(Message::Continue))
                .secondary_action(button(text(t!("generic.back"))).on_press(Message::Back))
                .into(),
            State::DeviceName => form()
                .title(t!("add-device.title.device-name"))
                .control(
                    text_input(&t!("add-device.input.device-name"), &self.device_name)
                        .id("device-name")
                        .on_input(|input| Message::Input(Input::DeviceName(input)))
                        .on_submit(Message::Continue),
                )
                .primary_action(button(text(t!("generic.continue"))).on_press(Message::Continue))
                .secondary_action(button(text(t!("generic.back"))).on_press(Message::Back))
                .into(),
            State::ConfirmCode => form()
                .title(t!("add-device.title.confirm-code"))
                .control(
                    text_input(&t!("add-device.input.confirm-code"), &self.confirm_code)
                        .id("confirm-code")
                        .on_input(|input| Message::Input(Input::ConfirmCode(input)))
                        .on_submit(Message::Continue),
                )
                .primary_action(button(text(t!("generic.continue"))).on_press(Message::Continue))
                .secondary_action(button(text(t!("generic.back"))).on_press(Message::Back))
                .into(),
            State::Success => form()
                .title(t!("add-device.title.success"))
                .control(text(t!("add-device.success")))
                .primary_action(button(text(t!("generic.back"))).on_press(Message::Exit))
                .into(),
        }
    }
}
