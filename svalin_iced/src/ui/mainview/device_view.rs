use std::sync::Arc;

use iced::{
    Length, Task,
    alignment::Horizontal,
    widget::{
        self, button, center, column, container, row, rule, scrollable, space, text, text_input,
    },
};
use svalin::client::{Client, state::ClientState};
use svalin_client_store::persistent::{SvalinMetaInfo, SvalinReport};
use svalin_pki::SpkiHash;

use crate::{
    Element, bootstrap,
    ui::widgets::{card, header},
};

mod meta_display;

#[derive(Debug, Clone)]
pub enum Message {
    Back,
    MetaDisplay(meta_display::Message),
    UpdateUrlChanged(String),
    Update,
}

pub enum Action {
    None,
    Back,
    Run(Task<Message>),
}

pub struct DeviceView {
    spki_hash: SpkiHash,
    meta_display: meta_display::MetaDisplay,
    update_url: String,
}

const PLACEHOLDER_META: &'static SvalinMetaInfo = &SvalinMetaInfo {
    updated_at: 0,
    name: String::new(),
    group: String::new(),
    notes: String::new(),
};

impl DeviceView {
    pub fn new(spki_hash: SpkiHash) -> Self {
        Self {
            spki_hash,
            update_url: String::new(),
            meta_display: meta_display::MetaDisplay::new(),
        }
    }

    pub fn update<'a>(
        &'a mut self,
        message: Message,
        client_state: &'a ClientState,
        client: &Arc<Client>,
    ) -> Action {
        let Some(persistent) = client_state.persistent().get(&self.spki_hash) else {
            if let Message::Back = message {
                return Action::Back;
            } else {
                return Action::None;
            }
        };

        match message {
            Message::Back => Action::Back,
            Message::MetaDisplay(message) => {
                let meta = persistent.meta_info().unwrap_or(&PLACEHOLDER_META);
                let Some(new_meta) = self.meta_display.update(message, &meta) else {
                    return Action::None;
                };

                let client = client.clone();
                let spki_hash = self.spki_hash.clone();
                Action::Run(
                    Task::future(async move {
                        if let Err(err) = client.device(spki_hash).update_metainfo(new_meta).await {
                            // TODO: Show error to user, probably refactor out the whole meta info gui
                            tracing::error!(?err, "Failed to update meta info");
                        }
                    })
                    .discard(),
                )
            }
            Message::UpdateUrlChanged(update_url) => {
                self.update_url = update_url;
                Action::None
            }
            Message::Update => {
                if self.update_url.is_empty() {
                    return Action::None;
                }

                let client = client.clone();
                let spki_hash = self.spki_hash.clone();
                let update_url = self.update_url.clone();

                Action::Run(
                    Task::future(async move {
                        if let Err(err) = client.device(spki_hash).update_agent(update_url).await {
                            // TODO: Show error to user, probably refactor out the whole meta info gui
                            tracing::error!(?err, "Failed to update device");
                        }
                    })
                    .discard(),
                )
            }
        }
    }

    pub fn view<'a>(&'a self, client_state: &'a ClientState) -> Element<'a, Message> {
        let Some(persistent) = client_state.persistent().get(&self.spki_hash) else {
            return center("Device not yet available").into();
        };

        let meta = persistent.meta_info().unwrap_or(&PLACEHOLDER_META);

        scrollable(
            column![
                if client_state.agent_online(&self.spki_hash) {
                    Some(
                        card(
                            column![
                                text_input("Update URL", &self.update_url)
                                    .on_input(Message::UpdateUrlChanged),
                                button("Update").on_press_maybe(if self.update_url.is_empty() {
                                    None
                                } else {
                                    Some(Message::Update)
                                },)
                            ]
                            .spacing(10),
                        )
                        .title("Actions"),
                    )
                } else {
                    None
                },
                self.meta_display.view(&meta).map(Message::MetaDisplay),
                if let Some(report) = persistent.report() {
                    Some(device_report(report))
                } else {
                    None
                },
            ]
            .padding(50)
            .spacing(50),
        )
        .into()
    }

    pub fn header<'a>(&'a self, client_state: &'a ClientState) -> Element<'a, Message> {
        let Some(persistent) = client_state.persistent().get(&self.spki_hash) else {
            return header(widget::space()).on_back(Message::Back).into();
        };

        header(text(persistent.name()))
            .on_back(Message::Back)
            .into()
    }
}

fn device_report(svalin_report: &SvalinReport) -> Element<'_, Message> {
    let report = &svalin_report.system_report;
    card(
        column![
            row![
                "Agent Version:",
                space::horizontal(),
                svalin_report.current_version_identifier.as_str()
            ],
            row![
                "Hostname:",
                space::horizontal(),
                report.hostname.as_ref().map(widget::text)
            ],
            row![
                "OS Family:",
                space::horizontal(),
                text!("{}", report.os_family)
            ],
            row![
                "OS:",
                space::horizontal(),
                report.os.as_ref().map(widget::text)
            ],
            row![
                "Kernel Version:",
                space::horizontal(),
                text(&report.kernel_version)
            ],
            rule::horizontal(2),
            row!["CPU Brand:", space::horizontal(), text(&report.cpu.brand)],
            row!["CPU Model:", space::horizontal(), text(&report.cpu.model)],
            row![
                "CPU Architecture:",
                space::horizontal(),
                text(&report.cpu.arch)
            ],
            row![
                "Physical CPU Cores:",
                space::horizontal(),
                report.cpu.cores.map(|c| text!("{}", c))
            ],
            row![
                "CPU Threads:",
                space::horizontal(),
                text!("{}", report.cpu.threads)
            ],
            row![
                "Total Memory:",
                space::horizontal(),
                text!("{}", report.total_memory)
            ],
            row![
                "Total Swap:",
                space::horizontal(),
                text!("{}", report.total_swap)
            ],
            widget::grid(report.disks.iter().map(|disk| {
                container(
                    column![
                        bootstrap::hdd().size(50).center(),
                        text!("{} ({})", &disk.name, &disk.mount_point),
                        widget::progress_bar(
                            0.0..=disk.total_space as f32,
                            (disk.total_space - disk.available_space) as f32
                        ),
                        text(&disk.file_system),
                    ]
                    .spacing(10)
                    .align_x(Horizontal::Center)
                    .padding(20)
                    .width(Length::Fill),
                )
                .center(100)
                .style(container::rounded_box)
                .into()
            }))
            .spacing(10)
        ]
        .spacing(10),
    )
    .title("System Report")
    .into()
}
