use std::{
    fs::{create_dir, create_dir_all},
    marker::PhantomData,
    ops::Deref,
    path::PathBuf,
};

use iced::{
    advanced::graphics::futures::backend::native::tokio, futures::channel::mpsc, widget::{button, column, iced, text, text_input}, Element, Task
};
use rust_embed::Embed;
use svalin_iced::ui::widgets::form;

#[derive(Debug, Clone)]
pub enum Message {
    Continue,
    Back,
    Path(String),
    CopyUpdate(CopyUpdate),
}

#[derive(Debug, Clone)]
pub enum Step {
    Greeting,
    Path,
    Installing,
    Finished,
    Error {
        message: String,
        previous: Option<Box<Step>>,
    },
}

pub struct Settings {
    install_path: String,
}

pub struct Installer<MainAsset> {
    app_name: String,
    step: Step,
    settings: Settings,
    main_asset: PhantomData<MainAsset>,
}

pub fn installer<MainAsset>(app_name: String, default_path: String) -> Installer<MainAsset> {
    Installer {
        app_name,
        step: Step::Greeting,
        settings: Settings {
            install_path: default_path,
        },
        main_asset: PhantomData,
    }
}

impl<MainAsset> Installer<MainAsset>
where
    MainAsset: Embed,
{
    pub fn title(&self) -> String {
        t!("title", app_name = self.app_name).to_string()
    }

    pub fn view(&self) -> Element<Message> {
        match &self.step {
            Step::Greeting => form()
                .control(text(t!("install", app_name = self.app_name)))
                .primary_action(button(text(t!("generic.continue"))).on_press(Message::Continue))
                .into(),
            Step::Path => form()
                .control(column!(
                    text(t!("install_path")),
                    text_input("", &self.settings.install_path).on_input(Message::Path),
                ))
                .primary_action(button(text(t!("generic.continue"))).on_press(Message::Continue))
                .secondary_action(button(text(t!("generic.back"))).on_press(Message::Back))
                .into(),
            Step::Installing => todo!(),
            Step::Finished => todo!(),
            Step::Error { message, .. } => form()
                .control(text(message))
                .primary_action(button(text(t!("generic.back"))).on_press(Message::Back))
                .into(),
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Continue => match self.step {
                Step::Greeting => {
                    self.step = Step::Path;
                    Task::none()
                }
                Step::Path => {
                    let dir_create = create_dir(&self.settings.install_path);

                    let error = if let Err(err) = dir_create {
                        match err.kind() {
                            std::io::ErrorKind::NotFound => Some(
                                t!("error.path_not_found", path = self.settings.install_path)
                                    .to_string(),
                            ),
                            std::io::ErrorKind::AlreadyExists => None,
                            _ => Some(format!("{}", err)),
                        }
                    } else {
                        None
                    };

                    if let Some(error) = error {
                        self.step = Step::Error {
                            message: error,
                            previous: Some(Box::new(self.step.clone())),
                        };
                        Task::none()
                    } else {
                        self.step = Step::Installing;
                        // TODO: install
                        let (send, recv) = mpsc::channel(2);

                        tokio::spawn(async move {
                            
                        });

                        Task::stream(recv)
                    }
                }
                Step::Installing => todo!(),
                Step::Finished => todo!(),
                Step::Error { .. } => Task::none(),
            },
            Message::Back => match &self.step {
                Step::Greeting => Task::none(),
                Step::Path => {
                    self.step = Step::Greeting;
                    Task::none()
                }
                Step::Installing => todo!(),
                Step::Finished => todo!(),
                Step::Error { previous, .. } => {
                    if let Some(previous) = previous {
                        self.step = previous.as_ref().clone();
                        Task::none()
                    } else {
                        self.step = Step::Greeting;
                        Task::none()
                    }
                }
            },
            Message::Path(path) => {
                self.settings.install_path = path;
                Task::none()
            }
            Message::CopyUpdate(copy_update) => todo!(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum CopyUpdate {
    Error(String),
    Finished,
}
