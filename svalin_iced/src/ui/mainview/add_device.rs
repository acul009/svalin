use std::sync::Arc;

use iced::{
    Task,
    widget::{button, column, operation, text_input},
};
use svalin::client::Client;
use svalin_pki::{Certificate, SpkiHash};

use crate::{
    Element,
    ui::widgets::{dialog, error_display, loading},
};

#[derive(Debug, Clone)]
pub enum Message {
    JoinCode(String),
    ConfirmCode(String),

    ConnectToDevice,
    Cancel,
    Done(Certificate),
    Error(Arc<anyhow::Error>),
}

pub enum Screen {
    Loading(String),
    Error(Arc<anyhow::Error>),
    Input,
}

pub struct AddDevice {
    screen: Screen,
    join_code: String,
    confirm_code: String,
    _handle: Option<iced::task::Handle>,
}

pub enum Action {
    None,
    Run(Task<Message>),
    Close,
    Done(SpkiHash),
}

impl AddDevice {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                screen: Screen::Input,
                join_code: String::new(),
                confirm_code: String::new(),
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
            Message::Error(err) => {
                if let Screen::Error(_) = self.screen {
                    return Action::None;
                }
                self.screen = Screen::Error(err);
                Action::None
            }
            Message::ConnectToDevice => {
                let Screen::Input = &self.screen else {
                    return Action::None;
                };

                let client = client.clone();
                let join_code = self.join_code.clone();
                let confirm_code = self.confirm_code.clone();

                let (add_task, handle) = Task::future(async move {
                    match client.add_agent_with_code(join_code, confirm_code).await {
                        Ok(certificate) => Message::Done(certificate),
                        Err(err) => Message::Error(Arc::new(err)),
                    }
                })
                .abortable();

                self._handle = Some(handle.abort_on_drop());

                self.screen = Screen::Loading(t!("add-device.connecting").to_string());

                Action::Run(add_task)
            }
            Message::Done(certificate) => Action::Done(certificate.spki_hash().clone()),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        match &self.screen {
            Screen::Error(err) => error_display(err)
                .on_close(Message::Cancel)
                .overlay()
                .into(),
            Screen::Loading(message) => dialog(loading(message))
                .title("Adding Device...")
                .overlay()
                .into(),
            Screen::Input => dialog(
                column![
                    text_input(t!("add-device.input.join-code"), &self.join_code)
                        .on_input(Message::JoinCode)
                        .id("join_code"),
                    text_input(t!("add-device.input.confirm-code"), &self.confirm_code)
                        .id("confirm_code")
                        .on_input(Message::ConfirmCode)
                        .on_submit(Message::ConnectToDevice),
                ]
                .spacing(10),
            )
            .title("Add Device")
            .on_close(Message::Cancel)
            .button(
                button(iced::widget::text(t!("generic.continue")))
                    .on_press(Message::ConnectToDevice),
            )
            .overlay()
            .into(),
        }
    }
}
