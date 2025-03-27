use std::sync::Arc;

use anyhow::anyhow;
use iced::{
    Task,
    widget::{button, text, text_input},
};
use svalin::client::{Client, Login, LoginError};

use crate::{
    Element,
    ui::{
        types::error_display_info::ErrorDisplayInfo,
        widgets::{form, loading},
    },
};

#[derive(Clone, Debug)]
pub enum Message {
    Username(String),
    Password(String),
    Totp(String),
    Error(ErrorDisplayInfo<Arc<anyhow::Error>>),
    OpenProfile(Arc<Client>),
    Continue,
    Back,
    WrongPassword,
    InvalidTotp,
}

pub enum Action {
    None,
    Exit(String),
    OpenProfile(Arc<Client>),
    Run(Task<Message>),
}

pub enum State {
    LoginForm,
    Loading(String),
    Error(ErrorDisplayInfo<Arc<anyhow::Error>>),
    WrongPassword,
    InvalidTotp,
}

pub struct LoginDialog {
    login: Option<Arc<Login>>,
    address: String,
    state: State,
    username: String,
    password: String,
    current_totp: String,
}

impl LoginDialog {
    pub fn start(login: Arc<Login>) -> (Self, Task<Message>) {
        (
            Self {
                address: login.address().to_string(),
                login: Some(login),
                state: State::LoginForm,
                username: String::new(),
                password: String::new(),
                current_totp: String::new(),
            },
            text_input::focus("username"),
        )
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Username(username) => {
                self.username = username;
                Action::None
            }
            Message::Password(password) => {
                self.password = password;
                Action::None
            }
            Message::Totp(totp) => {
                self.current_totp = totp;
                Action::None
            }
            Message::Error(error) => {
                self.state = State::Error(error);
                Action::None
            }
            Message::WrongPassword => {
                self.state = State::WrongPassword;
                Action::None
            }
            Message::InvalidTotp => {
                self.state = State::InvalidTotp;
                Action::None
            }
            Message::OpenProfile(client) => Action::OpenProfile(client),
            Message::Continue => match &self.state {
                State::LoginForm => match self.login.take() {
                    None => {
                        self.state = State::Error(ErrorDisplayInfo::new(
                            Arc::new(anyhow!("login already used")),
                            "login already used",
                        ));
                        return Action::None;
                    }
                    Some(login) => {
                        let login = match Arc::into_inner(login) {
                            None => return Action::None,
                            Some(login) => login,
                        };

                        self.state = State::Loading(t!("profile-picker.login-loading").to_string());
                        let username = self.username.clone();
                        let password = self.password.clone().into_bytes();
                        let totp = self.current_totp.clone();
                        Action::Run(Task::future(async move {
                            let login_result = login.login(username, password.clone(), totp).await;
                            match login_result {
                                Err(LoginError::WrongPassword) => Message::WrongPassword,
                                Err(LoginError::InvalidTotp) => Message::InvalidTotp,
                                Err(err) => Message::Error(ErrorDisplayInfo::new(
                                    Arc::new(anyhow!(err)),
                                    t!("profile-picker.error.login"),
                                )),
                                Ok(new_profile) => {
                                    match Client::open_profile_string(new_profile, password).await {
                                        Ok(client) => Message::OpenProfile(Arc::new(client)),
                                        Err(e) => Message::Error(ErrorDisplayInfo::new(
                                            e.into(),
                                            t!("profile-picker.error.unlock"),
                                        )),
                                    }
                                }
                            }
                        }))
                    }
                },
                _ => Action::None,
            },
            Message::Back => match &self.state {
                State::LoginForm | State::Error(_) | State::WrongPassword | State::InvalidTotp => {
                    Action::Exit(self.address.clone())
                }
                _ => Action::None,
            },
        }
    }

    pub fn view(&self) -> Element<Message> {
        match &self.state {
            State::Loading(message) => loading(message).into(),
            State::Error(display_info) => display_info.view().on_close(Message::Back).into(),
            State::LoginForm => form()
                .title(t!("profile-picker.login"))
                .control(
                    text_input(&t!("generic.username"), &self.username)
                        .id("username")
                        .on_input(|input| Message::Username(input)),
                )
                .control(
                    text_input(&t!("generic.password"), &self.password)
                        .secure(true)
                        .on_input(|input| Message::Password(input)),
                )
                .control(
                    text_input(&t!("profile-picker.input.totp"), &self.current_totp)
                        .id("totp")
                        .on_input(|input| Message::Totp(input))
                        .on_submit(Message::Continue),
                )
                .primary_action(button(text(t!("generic.continue"))).on_press(Message::Continue))
                .secondary_action(button(text(t!("generic.back"))).on_press(Message::Back))
                .into(),
            State::WrongPassword => form()
                .title(t!("profile-picker.wrong-password"))
                .primary_action(button(text(t!("generic.back"))).on_press(Message::Back))
                .into(),
            State::InvalidTotp => form()
                .title(t!("profile-picker.invalid-totp"))
                .primary_action(button(text(t!("generic.back"))).on_press(Message::Back))
                .into(),
        }
    }
}
