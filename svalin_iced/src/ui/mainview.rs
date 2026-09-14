use std::{borrow::Cow, process, sync::Arc, time::Duration};

use anyhow::anyhow;
use iced::{
    Length, Subscription, Task,
    widget::{button, column, row, scrollable, stack, text},
};
use svalin::client::{
    Client,
    state::{ClientState, Update, warning},
};
use svalin_pki::SpkiHash;
use tokio::sync::broadcast;
use tokio_stream::{StreamExt, wrappers::BroadcastStream};

use crate::{
    Element, bootstrap,
    ui::{
        ERROR_COLOR, INFO_COLOR, WARNING_COLOR,
        widgets::{error_display, header, icon_button, loading, toast},
    },
    util::human_i_bytes,
};

mod add_device;
mod device_list;
mod device_view;
mod tunnel_menu;

#[derive(Debug, Clone)]
pub enum Message {
    InitState(Arc<(ClientState, broadcast::Receiver<Update>)>),
    Error(Arc<anyhow::Error>),
    UpdateState(Update),
    OpenAddDevice,
    SelectDevice(SpkiHash),
    AddDevice(add_device::Message),
    DeviceView(device_view::Message),
    TunnelMenu(tunnel_menu::Message),
    OpenTerminal(
        Arc<(
            tokio::sync::mpsc::Sender<async_pty::TerminalInput>,
            tokio::sync::mpsc::Receiver<Vec<u8>>,
        )>,
    ),
    CloseError,
    Context(Context),
}

pub enum Action {
    None,
    Run(Task<Message>),
    OpenTerminal(
        Arc<(
            tokio::sync::mpsc::Sender<async_pty::TerminalInput>,
            tokio::sync::mpsc::Receiver<Vec<u8>>,
        )>,
    ),
}

enum Screen {
    Loading(String),
    DeviceList,
    AddDevice(add_device::AddDevice),
    DeviceView(device_view::State),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Context {
    None,
    Warnings,
    Tunnels,
}

pub struct MainView {
    screen: Screen,
    state: ClientState,
    context: Context,
    tunnel_menu: tunnel_menu::TunnelMenu,
    client: Arc<Client>,
    error: Option<Arc<anyhow::Error>>,
    update_abort_handle: Option<iced::task::Handle>,
}

impl MainView {
    pub fn new(client: Arc<Client>) -> (Self, Task<Message>) {
        let client2 = client.clone();
        (
            Self {
                screen: Screen::Loading(t!("device-list.loading").to_string()),
                state: ClientState::empty(),
                context: Context::None,
                tunnel_menu: tunnel_menu::TunnelMenu::new(),
                client,
                error: None,
                update_abort_handle: None,
            },
            Task::future(async move {
                match client2.subscribe_state().await {
                    Ok(state) => Message::InitState(Arc::new(state)),
                    Err(error) => Message::Error(Arc::new(error)),
                }
            }),
        )
    }

    #[must_use]
    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Error(error) => {
                self.error = Some(error);
                Action::None
            }
            Message::CloseError => {
                self.error = None;
                Action::None
            }
            Message::InitState(state) => {
                let (state, receiver) = Arc::into_inner(state).unwrap();
                self.state = state;
                self.screen = Screen::DeviceList;

                let updates = BroadcastStream::new(receiver)
                    .map_while(|update| update.ok().map(Message::UpdateState));
                let (update_task, abort_handle) = Task::stream(updates).abortable();
                self.update_abort_handle = Some(abort_handle.abort_on_drop());

                Action::Run(update_task)
            }
            Message::UpdateState(update) => {
                tracing::trace!("got update: {update:?}");
                self.state.update(update);
                Action::None
            }
            Message::SelectDevice(spki_hash) => {
                self.screen = Screen::DeviceView(device_view::State::new(spki_hash));
                Action::None
            }
            Message::OpenAddDevice => {
                let (add_device, task) = add_device::AddDevice::new();
                self.screen = Screen::AddDevice(add_device);
                Action::Run(task.map(Message::AddDevice))
            }
            Message::AddDevice(message) => {
                let Screen::AddDevice(add_device) = &mut self.screen else {
                    return Action::None;
                };

                match add_device.update(message, &self.client) {
                    add_device::Action::None => Action::None,
                    add_device::Action::Close => {
                        self.screen = Screen::DeviceList;
                        Action::None
                    }
                    add_device::Action::Done(spki_hash) => {
                        self.screen = Screen::DeviceView(device_view::State::new(spki_hash));
                        Action::None
                    }
                    add_device::Action::Run(task) => Action::Run(task.map(Message::AddDevice)),
                }
            }
            Message::DeviceView(message) => {
                let Screen::DeviceView(device_view) = &mut self.screen else {
                    return Action::None;
                };
                let action = device_view.update(message, &self.state, &self.client);
                match action {
                    device_view::Action::None => Action::None,
                    device_view::Action::Back => {
                        self.screen = Screen::DeviceList;
                        Action::None
                    }
                    device_view::Action::OpenTerminal(spki_hash) => {
                        let client = self.client.clone();
                        Action::Run(Task::future(async move {
                            // TODO: Handle error
                            Message::OpenTerminal(Arc::new(
                                client.device(spki_hash).open_terminal().await.unwrap(),
                            ))
                        }))
                    }
                    device_view::Action::Run(task) => Action::Run(task.map(Message::DeviceView)),
                }
            }
            Message::OpenTerminal(arc) => Action::OpenTerminal(arc),
            Message::TunnelMenu(message) => match self.tunnel_menu.update(message, &self.client) {
                tunnel_menu::Action::None => Action::None,
                tunnel_menu::Action::SelectDevice(device) => {
                    self.screen = Screen::DeviceView(device_view::State::new(device));
                    Action::None
                }
            },
            Message::Context(context) => {
                self.context = context;
                Action::None
            }
        }
    }

    pub fn view(&self) -> crate::Element<'_, Message> {
        let screen = match &self.screen {
            Screen::Loading(text) => loading(text).into(),
            Screen::DeviceList => device_list::DeviceList::new(&self.state)
                .on_new(Message::OpenAddDevice)
                .on_select(Message::SelectDevice)
                .into(),
            Screen::AddDevice(add_device) => stack![
                device_list::DeviceList::new(&self.state),
                add_device.view().map(Message::AddDevice)
            ]
            .into(),
            Screen::DeviceView(device_view) => {
                device_view.view(&self.state).map(Message::DeviceView)
            }
        };

        if let Some(error) = &self.error {
            stack![
                screen,
                error_display(error).on_close(Message::CloseError).overlay()
            ]
            .into()
        } else {
            screen
        }
    }

    pub fn header(&self) -> header::Header<'_, Message> {
        let header = match &self.screen {
            Screen::DeviceView(device_view) => {
                device_view.header(&self.state).map(Message::DeviceView)
            }
            _ => header(iced::widget::void()),
        };

        let severity = self.state.warnings().highest_severity();

        header
            .action(
                icon_button(bootstrap::ethernet())
                    .tooltip(text(t!("tunnel.menu.show")))
                    .on_press(Message::Context(Context::Tunnels)),
            )
            .action(
                icon_button(
                    bootstrap::exclamation_triangle_fill().color_maybe(severity.map(|severity| {
                        match severity {
                            warning::Severity::High => ERROR_COLOR,
                            warning::Severity::Medium => WARNING_COLOR,
                            warning::Severity::Low => INFO_COLOR,
                        }
                    })),
                )
                .tooltip(text(t!("warnings.show")))
                .on_press(Message::Context(Context::Warnings)),
            )
            .action(icon_button(bootstrap::x_lg()).on_press(Message::Context(Context::None)))
    }

    pub fn context(&self) -> Option<crate::Element<'_, Message>> {
        match &self.context {
            Context::None => None,
            Context::Tunnels => Some(self.tunnel_menu.view(&self.state).map(Message::TunnelMenu)),
            Context::Warnings => Some(
                scrollable(
                    column(self.state.warnings().warnings().map(|warning| {
                        Element::from(toast(
                            match warning.severity() {
                                warning::Severity::High => toast::Kind::Error,
                                warning::Severity::Medium => toast::Kind::Warning,
                                warning::Severity::Low => toast::Kind::Info,
                            },
                            match warning {
                                warning::Warning::Device(spki_hash, warning) => {
                                    let name = self
                                        .state
                                        .persistent()
                                        .devices()
                                        .get(spki_hash)
                                        .map(|d| d.name())
                                        .unwrap_or_else(|| Cow::Owned(spki_hash.to_string()));

                                    let mut actions = row![
                                        button(text(t!("warnings.device.action")))
                                            .on_press(Message::SelectDevice(spki_hash.clone()))
                                    ]
                                    .spacing(10);

                                    let message = match warning {
                                        warning::Device::DeviceGroupBroken(reason) => {
                                            actions = actions.push(
                                                button(text(t!("generic.details"))).on_press(
                                                    Message::Error(Arc::new(anyhow!(
                                                        reason.to_owned()
                                                    ))),
                                                ),
                                            );
                                            text(t!("warnings.device.group-broken"))
                                        }
                                        warning::Device::DiskSpaceLow { disk, free, total } => {
                                            text(t!(
                                                "warnings.device.disk-space-low",
                                                "disk" => disk,
                                                "free" => human_i_bytes(*free),
                                                "total" => human_i_bytes(*total)
                                            ))
                                        }
                                        warning::Device::MissingName => {
                                            text(t!("warnings.device.missing-name"))
                                        }
                                        warning::Device::BitlockerActive { drive, .. } => {
                                            text!("Drive {drive} has bitlocker active!")
                                        }
                                        warning::Device::PMGAttachmentQuarantine(count) => {
                                            text!("There are {count} mails in the attachment quarantine")
                                        }
                                        warning::Device::PMGVirusQuarantine(count) => {
                                            text!("There are {count} mails in the virus quarantine")
                                        }
                                        warning::Device::BackupOverdue { name, due_at } => {
                                            text(t!(
                                                "warnings.device.backup-overdue",
                                                "name" => name,
                                                "due_at" => format_warning_timestamp(*due_at)
                                            ))
                                        }
                                        warning::Device::BackupFailed {
                                            name,
                                            message,
                                            finished_at,
                                        } => {
                                            if let Some(finished_at) = finished_at {
                                                text(t!(
                                                    "warnings.device.backup-failed-at",
                                                    "name" => name,
                                                    "message" => message,
                                                    "finished_at" => format_warning_timestamp(*finished_at)
                                                ))
                                            } else {
                                                text(t!(
                                                    "warnings.device.backup-failed",
                                                    "name" => name,
                                                    "message" => message
                                                ))
                                            }
                                        }
                                    };
                                    Element::from(
                                        column![text(name).size(24), message, actions]
                                            .height(Length::Fit)
                                            .spacing(10),
                                    )
                                }
                            },
                        ))
                        .into()
                    }))
                    .padding(20)
                    .spacing(20),
                )
                .into(),
            ),
        }
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        Subscription::none()
    }

    pub(crate) fn shutdown(self) -> Task<()> {
        let client = self.client;

        Task::future(async move {
            if let Err(err) = client.close(Duration::from_secs(3)).await {
                tracing::error!("{err:#}");
                process::exit(1);
            }
        })
    }
}

fn format_warning_timestamp(timestamp: u64) -> String {
    i64::try_from(timestamp)
        .ok()
        .and_then(chrono::DateTime::from_timestamp_secs)
        .map(|time| {
            time.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M %:z")
                .to_string()
        })
        .unwrap_or_else(|| t!("warnings.device.unknown-time").into_owned())
}
