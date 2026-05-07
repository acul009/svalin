use std::{mem, sync::Arc};

use anyhow::anyhow;
use iced::{
    Task,
    widget::{self, button, operation, text_input},
};
use svalin::client::Client;
use svalin_pki::Certificate;
use tokio::sync::oneshot;

use crate::{
    Element,
    ui::widgets::{dialog, error_display, loading},
};

#[derive(Debug, Clone)]
pub enum Message {
    JoinCode(String),
    Name(String),
    Group(String),
    ConfirmCode(String),

    ConnectToDevice,
    Cancel,
    Done(Certificate),
    WaitForConfirm(Arc<oneshot::Sender<String>>),
    Error(Arc<anyhow::Error>),
    SwitchToConfirm,
    Confirm,
}

pub enum Screen {
    Loading(String),
    Error(Arc<anyhow::Error>),
    JoinCode,
    Name,
    Confirm,
}

pub struct AddDevice {
    screen: Screen,
    join_code: String,
    confirm_code: String,
    name: String,
    group: String,
    confirm_sender: Option<oneshot::Sender<String>>,
    _handle: Option<iced::task::Handle>,
}

pub enum Action {
    None,
    Run(Task<Message>),
    Close,
    Done(Certificate),
}

impl AddDevice {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                screen: Screen::JoinCode,
                join_code: String::new(),
                confirm_code: String::new(),
                name: String::new(),
                group: String::new(),
                confirm_sender: None,
                _handle: None,
            },
            operation::focus("join_code"),
        )
    }

    pub fn update(&mut self, message: Message, client: &Arc<Client>) -> Action {
        match message {
            Message::Cancel => Action::Close,
            Message::JoinCode(join_code) => {
                self.join_code = join_code.chars().filter(|c| c.is_numeric()).collect();
                Action::None
            }
            Message::ConfirmCode(confirm_code) => {
                self.confirm_code = confirm_code.chars().filter(|c| c.is_numeric()).collect();
                Action::None
            }
            Message::Name(name) => {
                self.name = name;
                Action::None
            }
            Message::Group(group) => {
                self.group = group;
                Action::None
            }
            Message::Error(err) => {
                if let Screen::Error(_) = self.screen {
                    return Action::None;
                }
                self.screen = Screen::Error(err);
                Action::None
            }
            Message::ConnectToDevice => {
                let Screen::JoinCode = &self.screen else {
                    return Action::None;
                };

                let client = client.clone();

                let (send, recv) = oneshot::channel();
                let join_code = self.join_code.clone();
                tracing::debug!("sending join code: {join_code:?}");

                let (add_task, handle) = Task::future(async move {
                    match client.add_agent_with_code(join_code, send).await {
                        Ok(certificate) => Message::Done(certificate),
                        Err(err) => Message::Error(Arc::new(err)),
                    }
                })
                .abortable();

                self._handle = Some(handle.abort_on_drop());

                self.screen = Screen::Loading("Connecting via join code...".into());

                Action::Run(Task::batch([
                    add_task,
                    Task::perform(recv, |res| match res {
                        Ok(confirm) => Message::WaitForConfirm(Arc::new(confirm)),
                        Err(err) => Message::Error(Arc::new(anyhow!("error adding agent: {err}"))),
                    }),
                ]))
            }
            Message::WaitForConfirm(confirm) => {
                let confirm = Arc::into_inner(confirm).unwrap();
                self.confirm_sender = Some(confirm);
                self.screen = Screen::Name;
                Action::Run(operation::focus("device_name"))
            }
            Message::SwitchToConfirm => {
                self.screen = Screen::Confirm;
                Action::Run(operation::focus("confirm_code"))
            }
            Message::Confirm => {
                let Screen::Confirm = &self.screen else {
                    return Action::None;
                };

                let Some(confirm_sender) = self.confirm_sender.take() else {
                    return Action::None;
                };
                if let Err(_) = confirm_sender.send(self.confirm_code.clone()) {
                    self.screen = Screen::Error(Arc::new(anyhow!("ran into timeout")));
                } else {
                    self.screen = Screen::Loading("Adding device...".into());
                }

                Action::None
            }
            Message::Done(certificate) => Action::Done(certificate),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        match &self.screen {
            Screen::Error(err) => error_display(err).on_close(Message::Cancel).into(),
            Screen::Loading(message) => loading(message).into(),
            Screen::JoinCode => dialog()
                .control(
                    text_input("Join Code", &self.join_code)
                        .on_input(Message::JoinCode)
                        .on_submit(Message::ConnectToDevice)
                        .id("join_code"),
                )
                .button(button("Cancel").on_press(Message::Cancel))
                .button(button("Continue").on_press(Message::ConnectToDevice))
                .into(),
            Screen::Name => dialog()
                .control(
                    text_input("Device Name", &self.name)
                        .on_input(Message::Name)
                        .id("device_name"),
                )
                .control(
                    text_input("Group", &self.group)
                        .on_input(Message::Group)
                        .on_submit(Message::SwitchToConfirm),
                )
                .button(button("Cancel").on_press(Message::Cancel))
                .button(button("Continue").on_press(Message::SwitchToConfirm))
                .into(),
            Screen::Confirm => dialog()
                .control(
                    text_input("Confirm Code", &self.confirm_code)
                        .id("confirm_code")
                        .on_input(Message::ConfirmCode)
                        .on_submit(Message::Confirm),
                )
                .button(button("Cancel").on_press(Message::Cancel))
                .button(button("Continue").on_press(Message::Confirm))
                .into(),
        }
    }
}
