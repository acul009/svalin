use std::sync::Arc;

use iced::{
    Length, Task, Widget,
    widget::{button, column, row, scrollable, text},
};
use svalin::client::Client;
use svalin_pki::UnverifiedCertificate;

use crate::{
    ui::widgets::{card, loading},
    util::format_timestamp,
};

#[derive(Debug, Clone)]
pub enum Message {
    Refresh,
    Loaded(Result<Vec<UnverifiedCertificate>, Arc<anyhow::Error>>),
}

enum State {
    Idle,
    Loading,
    Ready(Vec<Session>),
    Failed(String),
}

struct Session {
    fingerprint: String,
    valid_from: String,
    expires_at: String,
    current: bool,
}

pub struct SessionsView {
    state: State,
}

impl SessionsView {
    pub fn new() -> Self {
        Self { state: State::Idle }
    }

    pub fn update(&mut self, message: Message, client: &Arc<Client>) -> Task<Message> {
        match message {
            Message::Refresh => {
                if matches!(self.state, State::Loading) {
                    return Task::none();
                }
                self.state = State::Loading;
                let client = client.clone();
                Task::perform(
                    async move { client.list_user_sessions().await.map_err(Arc::new) },
                    Message::Loaded,
                )
            }
            Message::Loaded(result) => {
                self.state = match result {
                    Ok(mut certificates) => {
                        certificates.sort_by_key(|certificate| {
                            (
                                certificate.spki_hash() != client.session_spki_hash(),
                                std::cmp::Reverse(certificate.valid_from()),
                            )
                        });
                        State::Ready(
                            certificates
                                .into_iter()
                                .map(|certificate| Session {
                                    current: certificate.spki_hash() == client.session_spki_hash(),
                                    fingerprint: certificate.spki_hash().to_string(),
                                    valid_from: format_timestamp(certificate.valid_from()),
                                    expires_at: format_timestamp(certificate.expires_at()),
                                })
                                .collect(),
                        )
                    }
                    Err(error) => State::Failed(format!("{error:#}")),
                };
                Task::none()
            }
        }
    }

    pub fn view(&self) -> impl Widget<Message> + '_ {
        let content = match &self.state {
            State::Idle | State::Loading => loading(t!("sessions.loading")).into_widget().boxed(),
            State::Failed(error) => column![text(t!("sessions.load-failed")), text(error),]
                .spacing(10)
                .boxed(),
            State::Ready(sessions) if sessions.is_empty() => text(t!("sessions.empty")).boxed(),
            State::Ready(sessions) => scrollable(
                column(sessions.iter().map(|session| {
                    card(
                        column![
                            text(&session.fingerprint).size(14),
                            row![text(t!("sessions.valid-from")), text(&session.valid_from)]
                                .spacing(10),
                            row![text(t!("sessions.expires-at")), text(&session.expires_at)]
                                .spacing(10),
                        ]
                        .spacing(8),
                    )
                    .title(text(if session.current {
                        t!("sessions.current")
                    } else {
                        t!("sessions.session")
                    }))
                    .into_widget()
                }))
                .spacing(16),
            )
            .height(Length::Fill)
            .boxed(),
        };

        column![
            button(text(t!("sessions.refresh"))).on_press_maybe(
                (!matches!(self.state, State::Loading)).then_some(Message::Refresh)
            ),
            content,
        ]
        .spacing(16)
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
    }
}
