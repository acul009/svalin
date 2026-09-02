use std::{path::PathBuf, sync::Arc};

use iced::{
    Task,
    widget::{button, column, progress_bar, row, stack, text},
};
use svalin::client::Client;
use svalin_pki::SpkiHash;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::{Element, ui::widgets::card};

#[derive(Debug, Clone)]
pub enum Message {
    SelectFile,
    DialogClosed(Option<PathBuf>),
    StartAgentUpdate,
    Progress(f32, Uuid),
    Error(String),
    Success,
}

pub enum Action {
    Run(Task<Message>),
    None,
}

enum Status {
    None,
    Updating(f32, Uuid),
    Success,
    Error(String),
}

impl Status {
    fn is_updating(&self) -> bool {
        matches!(self, Self::Updating(_, _))
    }
}

pub struct State {
    file_path: Option<PathBuf>,
    selecting: bool,
    status: Status,
}

impl State {
    pub fn new() -> Self {
        Self {
            file_path: None,
            selecting: false,
            status: Status::None,
        }
    }

    pub fn update(
        &mut self,
        message: Message,
        client: &Arc<Client>,
        spki_hash: &SpkiHash,
    ) -> Action {
        match message {
            Message::SelectFile => {
                if self.selecting {
                    return Action::None;
                }
                let current = self.file_path.clone();
                self.selecting = true;

                Action::Run(Task::future(async move {
                    let mut dialog = rfd::AsyncFileDialog::new();
                    if let Some(current) = current {
                        dialog = dialog
                            .set_directory(current.parent().expect("file should have a parent"));
                    }
                    let file = dialog.pick_file().await;
                    Message::DialogClosed(file.map(|file| file.path().to_path_buf()))
                }))
            }
            Message::DialogClosed(file_path) => {
                self.selecting = false;
                if let Some(file_path) = file_path {
                    self.file_path = Some(file_path);
                }
                Action::None
            }
            Message::StartAgentUpdate => {
                let Some(path) = self.file_path.clone() else {
                    return Action::None;
                };
                if self.status.is_updating() {
                    return Action::None;
                }
                let client = client.clone();
                let spki_hash = spki_hash.clone();
                let id = Uuid::new_v4();
                self.status = Status::Updating(0.0, id);
                let (send, recv) = mpsc::channel(10);

                Action::Run(Task::batch([
                    Task::future(async move {
                        if let Err(err) = client.device(spki_hash).update_agent(path, send).await {
                            Message::Error(err.to_string())
                        } else {
                            Message::Success
                        }
                    }),
                    Task::run(ReceiverStream::new(recv), move |progress| {
                        Message::Progress(progress, id)
                    }),
                ]))
            }
            Message::Error(err) => {
                self.status = Status::Error(err);
                Action::None
            }
            Message::Progress(progress, id) => {
                if let Status::Updating(prog, my_id) = &mut self.status {
                    if id == *my_id {
                        *prog = progress;
                    }
                }
                Action::None
            }
            Message::Success => {
                self.status = Status::Success;
                Action::None
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        card(
            column![
                row![
                    button("Select File").on_press(Message::SelectFile),
                    self.file_path
                        .as_ref()
                        .map(|path| text!("{}", path.display()))
                ],
                button("Update").on_press_maybe(
                    if self.file_path.is_some() && !self.status.is_updating() {
                        Some(Message::StartAgentUpdate)
                    } else {
                        None
                    }
                ),
                match &self.status {
                    Status::None => {
                        Element::from(iced::widget::void())
                    }
                    Status::Updating(progress, _) => {
                        progress_bar(0.0..=1.0, *progress).into()
                    }
                    Status::Success => {
                        stack![progress_bar(0.0..=1.0, 1.0), text("Update successful")].into()
                    }
                    Status::Error(err) => {
                        stack![
                            progress_bar(0.0..=1.0, 1.0),
                            text!("Update failed: {}", err)
                        ]
                        .into()
                    }
                }
            ]
            .spacing(10),
        )
        .into()
    }
}
