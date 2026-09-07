use std::sync::Arc;

use iced::{
    Length, Task,
    alignment::Vertical,
    widget::{self, center, column, container, row, rule, scrollable, space, stack, text},
};
use svalin::client::{
    Client,
    state::{ClientState, persistent},
};
use svalin_pki::SpkiHash;
use svalin_sysctl::sytem_report::Disk;

use crate::{
    Element,
    bootstrap::{self},
    ui::widgets::{card, device_icon, dialog, fact_list, header, icon_button, os_icon},
    util::human_i_bytes,
};

mod meta_display;
mod update;

#[derive(Debug, Clone)]
pub enum Message {
    Back,
    OpenTerminal,
    Overlay(Overlay),
    MetaDisplay(meta_display::Message),
    Update(update::Message),
    CloseOverlay,
}

pub enum Action {
    None,
    Back,
    OpenTerminal(SpkiHash),
    Run(Task<Message>),
}

pub struct State {
    spki_hash: SpkiHash,
    meta_display: meta_display::State,
    update: update::State,
    overlay: Option<Overlay>,
}

#[derive(Debug, Clone)]
pub enum Overlay {
    Update,
}

impl From<Overlay> for Message {
    fn from(overlay: Overlay) -> Self {
        Self::Overlay(overlay)
    }
}

const PLACEHOLDER_META: &'static persistent::MetaInfo = &persistent::MetaInfo {
    updated_at: 0,
    name: String::new(),
    group: String::new(),
    notes: String::new(),
};

impl State {
    pub fn new(spki_hash: SpkiHash) -> Self {
        Self {
            spki_hash,
            meta_display: meta_display::State::new(),
            update: update::State::new(),
            overlay: None,
        }
    }

    pub fn update<'a>(
        &'a mut self,
        message: Message,
        client_state: &'a ClientState,
        client: &Arc<Client>,
    ) -> Action {
        let Some(persistent) = client_state.persistent().devices().get(&self.spki_hash) else {
            if let Message::Back = message {
                return Action::Back;
            } else {
                return Action::None;
            }
        };

        match message {
            Message::Back => Action::Back,
            Message::Overlay(overlay) => {
                self.overlay = Some(overlay);
                Action::None
            }
            Message::CloseOverlay => {
                self.overlay = None;
                Action::None
            }
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
                            // TODO: Show error to user
                            tracing::error!(?err, "Failed to update meta info");
                        }
                    })
                    .discard(),
                )
            }
            Message::Update(message) => {
                match self.update.update(message, &client, &self.spki_hash) {
                    update::Action::Run(task) => Action::Run(task.map(Message::Update)),
                    update::Action::None => Action::None,
                }
            }
            Message::OpenTerminal => Action::OpenTerminal(self.spki_hash.clone()),
        }
    }

    pub fn view<'a>(&'a self, client_state: &'a ClientState) -> Element<'a, Message> {
        let Some(persistent) = client_state.persistent().devices().get(&self.spki_hash) else {
            return center("Device not yet available").into();
        };

        let meta = persistent.meta_info().unwrap_or(&PLACEHOLDER_META);
        let online = client_state.agent_online(&self.spki_hash);
        let col = column![
            online.then(|| agent_actions()),
            self.meta_display.view(&meta).map(Message::MetaDisplay),
            if let Some(report) = persistent.report() {
                Some(device_report(report))
            } else {
                None
            }
        ]
        .padding(50)
        .spacing(50);

        let overlay: Option<Element<Message>> =
            self.overlay.as_ref().map(|overlay| match overlay {
                Overlay::Update => dialog(self.update.view().map(Message::Update))
                    .title("Update Agent")
                    .on_close(Message::CloseOverlay)
                    .overlay()
                    .into(),
            });

        stack![scrollable(col), overlay].height(Length::Fill).into()
    }

    pub fn header<'a>(&'a self, client_state: &'a ClientState) -> header::Header<'a, Message> {
        let Some(persistent) = client_state.persistent().devices().get(&self.spki_hash) else {
            return header(widget::space()).on_back(Message::Back).into();
        };

        header(
            row![
                device_icon(&persistent.os(), client_state.agent_online(&self.spki_hash)).size(30),
                text(persistent.name()).size(24)
            ]
            .align_y(Vertical::Center)
            .spacing(20),
        )
        .on_back(Message::Back)
    }
}

fn agent_actions() -> Element<'static, Message> {
    card(
        row![
            icon_button(bootstrap::terminal())
                .size(50)
                .tooltip("Open Terminal")
                .on_press(Message::OpenTerminal),
            icon_button(bootstrap::download())
                .size(50)
                .tooltip("Update Device")
                .on_press(Overlay::Update.into())
        ]
        .padding(20)
        .spacing(20),
    )
    .padding(0)
    .title("Agent Actions")
    .into()
}

fn device_report(svalin_report: &persistent::Report) -> Element<'_, Message> {
    let report = &svalin_report.system_report;
    card(
        column![
            container(
                fact_list()
                    .entry(
                        "Agent Version:",
                        svalin_report.current_version_identifier.as_str(),
                    )
                    .entry("Hostname:", report.hostname.as_deref().unwrap_or_default())
                    .entry(
                        "OS:",
                        row![
                            report
                                .os
                                .as_ref()
                                .map(iced::widget::text)
                                .unwrap_or_else(|| text(report.os_family.to_string())),
                            os_icon(&report.os_family)
                        ]
                        .spacing(10)
                        .align_y(Vertical::Center)
                    )
                    .entry("Kernel Version:", report.kernel_version.as_str())
                    // .entry("CPU Brand:", report.cpu.brand.as_str())
                    .entry("CPU Model:", report.cpu.model.as_str())
                    // .entry("CPU Architecture:", report.cpu.arch.as_str())
                    .entry(
                        if report.cpu.cores.is_some() {
                            text("CPU Cores / Threads")
                        } else {
                            text("CPU Threads")
                        },
                        if let Some(cores) = report.cpu.cores {
                            text!("{} / {}", cores, report.cpu.threads)
                        } else {
                            text!("{}", report.cpu.threads)
                        }
                    )
                    .entry(
                        "Total Memory / Swap:",
                        text!(
                            "{} / {}",
                            human_i_bytes(report.total_memory),
                            human_i_bytes(report.total_swap)
                        )
                    )
            )
            .padding(30),
            rule::horizontal(2),
            container(crate::ui::widgets::list(report.disks.iter().map(disk)).entry_height(90))
                .padding(30),
        ]
        .spacing(15),
    )
    .title("System Report")
    .padding(0)
    .into()
}

fn disk<'a>(disk: &'a Disk) -> Element<'a, Message> {
    row![
        bootstrap::hdd().size(50).center(),
        column![
            row![
                text(&disk.mount_point).size(20),
                space::horizontal(),
                text(&disk.name)
            ]
            .padding([0, 20]),
            stack![
                widget::progress_bar(
                    0.0..=disk.total_space as f32,
                    (disk.total_space - disk.available_space) as f32
                )
                .girth(Length::Fill),
                row![
                    text!(
                        "{} / {} Free",
                        human_i_bytes(disk.available_space),
                        human_i_bytes(disk.total_space)
                    )
                    .align_y(Vertical::Center),
                    space::horizontal(),
                    text(&disk.file_system).align_y(Vertical::Center)
                ]
                .align_y(Vertical::Center)
                .height(Length::Fill)
                .padding([0, 20])
            ]
            .height(30)
        ]
        .spacing(10),
    ]
    .align_y(Vertical::Center)
    .height(Length::Fill)
    .spacing(20)
    .padding([0, 20])
    .into()
}
