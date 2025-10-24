use std::sync::Arc;

use iced::{
    Task,
    advanced::widget::{operate, operation::focusable::focus},
    widget::{button, operation, text, text_input},
};
use svalin::client::{Client, add_agent::WaitingForConfirmCode};

use crate::ui::{
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

pub enum Action {
    None,
    Exit,
    Run(Task<Message>),
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
            // focus("join-code").into(),
            iced::widget::operation::focus("join-code"),
        )
    }

    #[must_use]
    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Error(error) => {
                self.state = State::Error(error);
                Action::None
            }
            Message::Input(input) => {
                input.update(self);
                Action::None
            }
            Message::Success => {
                self.state = State::Success;
                Action::None
            }
            Message::Continue => match &mut self.state {
                State::Error(_) => Action::None,
                State::JoinCode => {
                    let join_code = self.join_code.clone();
                    let client = self.client.clone();
                    self.state = State::Loading(t!("add-device.connecting").to_string());
                    Action::Run(Task::future(async move {
                        let waiting = client.add_agent_with_code(join_code.clone()).await;

                        match waiting {
                            Err(err) => Message::Error(ErrorDisplayInfo::new(
                                Arc::new(err),
                                t!("add-device.error.join-code"),
                            )),
                            Ok(waiting) => Message::WaitingForDeviceName(Arc::new(waiting)),
                        }
                    }))
                }
                State::DeviceName => {
                    self.state = State::ConfirmCode;
                    Action::Run(iced::widget::operation::focus("confirm-code"))
                }
                State::ConfirmCode => match self.waiting.take() {
                    None => {
                        self.state = State::Error(ErrorDisplayInfo::new(
                            Arc::new(anyhow::anyhow!("waiting for device name")),
                            t!("add-device.error.join-code"),
                        ));
                        Action::None
                    }
                    Some(waiting) => {
                        let confirm_code = self.confirm_code.clone();
                        let device_name = self.device_name.clone();
                        self.state = State::Loading(t!("add-device.enrolling").to_string());

                        Action::Run(Task::future(async move {
                            let joined = waiting.confirm(confirm_code, device_name).await;

                            match joined {
                                Err(err) => Message::Error(ErrorDisplayInfo::new(
                                    Arc::new(err),
                                    t!("add-device.error.join-code"),
                                )),
                                Ok(_) => Message::Success,
                            }
                        }))
                    }
                },
                State::Success | State::Loading(_) => Action::None,
            },
            Message::Back => match &mut self.state {
                State::JoinCode => Action::Exit,
                State::Error(_) | State::DeviceName => {
                    self.state = State::JoinCode;
                    self.join_code = String::new();
                    self.confirm_code = String::new();
                    self.waiting = None;
                    Action::Run(iced::widget::operation::focus("join-code"))
                }
                State::ConfirmCode => {
                    self.state = State::DeviceName;
                    self.confirm_code = String::new();
                    Action::Run(iced::widget::operation::focus("device-name"))
                }
                State::Success | State::Loading(_) => Action::None,
            },
            Message::WaitingForDeviceName(waiting) => {
                let waiting = Arc::into_inner(waiting).unwrap();

                self.waiting = Some(waiting);
                self.state = State::DeviceName;

                Action::Run(iced::widget::operation::focus("device-name"))
            }
            Message::Exit => Action::Exit,
        }
    }

    pub fn view(&self) -> crate::Element<Message> {
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
                .button(button(text(t!("generic.back"))).on_press(Message::Back))
                .button(button(text(t!("generic.continue"))).on_press(Message::Continue))
                .into(),
            State::DeviceName => form()
                .title(t!("add-device.title.device-name"))
                .control(
                    text_input(&t!("add-device.input.device-name"), &self.device_name)
                        .id("device-name")
                        .on_input(|input| Message::Input(Input::DeviceName(input)))
                        .on_submit(Message::Continue),
                )
                .button(button(text(t!("generic.back"))).on_press(Message::Back))
                .button(button(text(t!("generic.continue"))).on_press(Message::Continue))
                .into(),
            State::ConfirmCode => form()
                .title(t!("add-device.title.confirm-code"))
                .control(
                    text_input(&t!("add-device.input.confirm-code"), &self.confirm_code)
                        .id("confirm-code")
                        .on_input(|input| Message::Input(Input::ConfirmCode(input)))
                        .on_submit(Message::Continue),
                )
                .button(button(text(t!("generic.back"))).on_press(Message::Back))
                .button(button(text(t!("generic.continue"))).on_press(Message::Continue))
                .into(),
            State::Success => form()
                .title(t!("add-device.title.success"))
                .control(text(t!("add-device.success")))
                .button(button(text(t!("generic.back"))).on_press(Message::Exit))
                .into(),
        }
    }
}
