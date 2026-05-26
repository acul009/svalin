use std::sync::Arc;

use iced::{
    Length, Task,
    alignment::Horizontal,
    widget::{
        self, button, center, column, container, row, rule, scrollable, space, text, text_input,
    },
};
use svalin::client::{Client, state::ClientState};
use svalin_client_store::persistent::SvalinMetaInfo;
use svalin_pki::{SpkiHash, get_current_timestamp};
use svalin_sysctl::sytem_report::SystemReport;

use crate::{
    Element, bootstrap,
    ui::widgets::{card, header},
};

#[derive(Debug, Clone)]
pub enum Message {
    Back,
    EditMeta,
    CancelEditMeta,
    ChangeName(String),
    ChangeGroup(String),
    ChangeNotes(String),
    SaveMeta,
}

pub enum Action {
    None,
    Back,
    Run(Task<Message>),
}

pub struct DeviceView {
    spki_hash: SpkiHash,
    edit_meta: Option<SvalinMetaInfo>,
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
            edit_meta: None,
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
            Message::EditMeta => {
                let meta = persistent.meta_info().unwrap_or(&PLACEHOLDER_META).clone();
                self.edit_meta = Some(meta);
                Action::None
            }
            Message::CancelEditMeta => {
                self.edit_meta = None;
                Action::None
            }
            Message::ChangeName(name) => {
                if let Some(meta) = &mut self.edit_meta {
                    meta.name = name;
                }
                Action::None
            }
            Message::ChangeGroup(group) => {
                if let Some(meta) = &mut self.edit_meta {
                    meta.group = group;
                }
                Action::None
            }
            Message::ChangeNotes(notes) => {
                if let Some(meta) = &mut self.edit_meta {
                    meta.notes = notes;
                }
                Action::None
            }
            Message::SaveMeta => {
                let Some(mut meta) = self.edit_meta.take() else {
                    return Action::None;
                };
                meta.updated_at = get_current_timestamp();
                let client = client.clone();
                let spki_hash = self.spki_hash.clone();
                Action::Run(
                    Task::future(async move {
                        if let Err(err) = client.device(spki_hash).update_metainfo(meta).await {
                            // TODO: Show error to user, probably refactor out the whole meta info gui
                            tracing::error!(?err, "Failed to update meta info");
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
                if let Some(meta) = &self.edit_meta {
                    card(
                        column![
                            row![
                                "Name:",
                                space::horizontal(),
                                text_input("", &meta.name).on_input(Message::ChangeName)
                            ],
                            row![
                                "Group:",
                                space::horizontal(),
                                text_input("", &meta.group).on_input(Message::ChangeGroup)
                            ],
                            row![
                                "Notes:",
                                space::horizontal(),
                                text_input("", &meta.notes).on_input(Message::ChangeNotes)
                            ],
                        ]
                        .spacing(10),
                    )
                    .title(row![
                        "Device Information",
                        space::horizontal(),
                        button(bootstrap::floppy()).on_press(Message::SaveMeta),
                        button(bootstrap::x_square()).on_press(Message::CancelEditMeta)
                    ])
                } else {
                    card(
                        column![
                            row!["Name:", space::horizontal(), text(&meta.name)],
                            row!["Group:", space::horizontal(), text(&meta.group)],
                            row!["Notes:", space::horizontal(), text(&meta.notes)],
                        ]
                        .spacing(10),
                    )
                    .title(row![
                        "Device Information",
                        space::horizontal(),
                        button(bootstrap::pencil()).on_press(Message::EditMeta)
                    ])
                },
                if let Some(report) = persistent.report() {
                    let report = &report.system_report;
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

fn device_report(report: &SystemReport) -> Element<'_, Message> {
    card(
        column![
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
