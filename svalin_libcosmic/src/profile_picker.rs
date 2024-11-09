use std::sync::Arc;

use anyhow::Result;
use cosmic::{
    iced_widget::{button, column, row},
    widget::{text, text_input},
    Element, Task,
};
use svalin::client::Client;

enum State {
    SelectProfile(Vec<String>),
    UnlockProfile { profile: String, password: String },
    Loading,
}

pub struct ProfilePicker {
    state: State,
}

#[derive(Debug, Clone)]
pub enum Message {
    Reset,
    SelectProfile(String),
    DeleteProfile(String),
    InputPassword(String),
    UnlockProfile,
    Profile(Arc<Client>),
}

impl ProfilePicker {
    pub fn new() -> Self {
        let profiles = Client::get_profiles().unwrap_or_else(|_| Vec::new());
        Self {
            state: State::SelectProfile(profiles),
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Reset => {
                let profiles = Client::get_profiles().unwrap_or_else(|_| Vec::new());

                self.state = State::SelectProfile(profiles);
            }
            Message::SelectProfile(profile) => {
                self.state = State::UnlockProfile {
                    profile,
                    password: String::new(),
                };
            }
            Message::InputPassword(password) => match &self.state {
                State::UnlockProfile { profile, .. } => {
                    self.state = State::UnlockProfile {
                        profile: profile.clone(),
                        password,
                    }
                }
                _ => (),
            },
            Message::DeleteProfile(_) => (),
            Message::UnlockProfile => {
                if let State::UnlockProfile { profile, password } = &self.state {
                    let profile = profile.clone();
                    let password = password.clone();

                    self.state = State::Loading;

                    return Task::future(async move {
                        let client = Client::open_profile_string(profile, password)
                            .await
                            .unwrap();

                        Message::Profile(Arc::new(client))
                    });
                }
            }
            Message::Profile(_) => unreachable!(),
        };

        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
        match &self.state {
            State::SelectProfile(profiles) => column(profiles.iter().map(|p| {
                button(text(p))
                    .on_press(Message::SelectProfile(p.clone()))
                    .into()
            }))
            .into(),
            State::UnlockProfile { profile, password } => column![
                text("Unlocking profile..."),
                text_input("Password", password)
                    .password()
                    .on_input(|input| Message::InputPassword(input))
                    .on_paste(|input| Message::InputPassword(input))
                    .on_submit(Message::UnlockProfile),
                text(password),
                row![
                    button(text("Cancel")).on_press(Message::Reset),
                    button(text("Unlock")).on_press(Message::UnlockProfile)
                ]
            ]
            .into(),
            State::Loading => text("Loading...").into(),
        }
    }
}
