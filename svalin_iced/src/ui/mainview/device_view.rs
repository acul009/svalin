use std::sync::Arc;

use iced::{
    Length, Task,
    alignment::Vertical,
    widget::{
        self, center, column, container, progress_bar, row, rule, scrollable, space, stack, text,
    },
};
use svalin::client::{
    Client,
    state::{ClientState, persistent},
    tunnel_manager::{TunnelConfig, TunnelDefinition},
};
use svalin_pki::SpkiHash;
use svalin_sysctl::system_report::Disk;
use url::Url;

use crate::{
    Element, bootstrap,
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
    OpenRemoteWebinterface(Url),
    RemoteWebinterfaceOpened { scheme: String, local_port: u16 },
    RefreshReport,
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
    TunnelOpenError,
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
            Message::OpenRemoteWebinterface(url) => {
                let client = client.clone();
                let spki_hash = self.spki_hash.clone();
                let Some((host, port)) = url.host_str().zip(url.port_or_known_default()) else {
                    return Action::None;
                };
                let scheme = url.scheme().to_string();
                let remote_host = format!("{}:{}", host, port);
                let tunnel = TunnelDefinition {
                    name: "Proxmox-VE".into(),
                    target: self.spki_hash.clone(),
                    config: TunnelConfig::Tcp {
                        local_port: None,
                        remote_host,
                    },
                };

                Action::Run(Task::future(async move {
                    match client.device(spki_hash).open_tunnel(tunnel).await {
                        Err(_err) => Message::Overlay(Overlay::TunnelOpenError),
                        Ok(local_port) => Message::RemoteWebinterfaceOpened { local_port, scheme },
                    }
                }))
            }
            Message::RemoteWebinterfaceOpened { scheme, local_port } => {
                open::that_in_background(format!("{scheme}://127.0.0.1:{local_port}"));
                Action::None
            }
            Message::RefreshReport => {
                let spki_hash = self.spki_hash.clone();
                let client = client.clone();
                Action::Run(
                    Task::future(
                        async move { client.device(spki_hash).request_system_report().await },
                    )
                    .discard(),
                )
            }
        }
    }

    pub fn view<'a>(&'a self, client_state: &'a ClientState) -> Element<'a, Message> {
        let Some(persistent) = client_state.persistent().devices().get(&self.spki_hash) else {
            return center(text(t!("device.unavailable"))).into();
        };

        let meta = persistent.meta_info().unwrap_or(&PLACEHOLDER_META);
        let online = client_state.agent_online(&self.spki_hash);
        let col = column![
            online.then(|| agent_actions(persistent)),
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
                    .title(text(t!("device.update.title")))
                    .on_close(Message::CloseOverlay)
                    .overlay()
                    .into(),
                Overlay::TunnelOpenError => dialog(text(t!("device.tunnel.open-error")))
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

fn agent_actions<'a>(device_state: &persistent::DeviceState) -> Element<'a, Message> {
    let extensions = device_state.report().map(|r| &r.system_report.extensions);
    let is_proxmox_ve = extensions
        .map(|e| e.proxmox_ve().is_some())
        .unwrap_or(false);
    let is_proxmox_bs = extensions
        .map(|e| e.proxmox_bs().is_some())
        .unwrap_or(false);
    let is_proxmox_mg = extensions
        .map(|e| e.proxmox_mg().is_some())
        .unwrap_or(false);
    card(
        row![
            icon_button(bootstrap::terminal())
                .size(50)
                .tooltip(text(t!("device.actions.open-terminal")))
                .on_press(Message::OpenTerminal),
            icon_button(bootstrap::download())
                .size(50)
                .tooltip(text(t!("device.actions.update")))
                .on_press(Overlay::Update.into()),
            is_proxmox_ve.then(|| {
                icon_button(bootstrap::window_fullscreen())
                    .size(50)
                    .tooltip(text(t!("device.actions.open-pve")))
                    .on_press(Message::OpenRemoteWebinterface(
                        "https://127.0.0.1:8006"
                            .parse()
                            .expect("hand coded urls have to parse"),
                    ))
            }),
            is_proxmox_bs.then(|| {
                icon_button(bootstrap::window_fullscreen())
                    .size(50)
                    .tooltip(text(t!("device.actions.open-pbs")))
                    .on_press(Message::OpenRemoteWebinterface(
                        "https://127.0.0.1:8007"
                            .parse()
                            .expect("hand coded urls have to parse"),
                    ))
            }),
            is_proxmox_mg.then(|| {
                icon_button(bootstrap::window_fullscreen())
                    .size(50)
                    .tooltip(text(t!("device.actions.open-pmg")))
                    .on_press(Message::OpenRemoteWebinterface(
                        "https://127.0.0.1:8006"
                            .parse()
                            .expect("hand coded urls have to parse"),
                    ))
            }),
        ]
        .padding(20)
        .spacing(20),
    )
    .padding(0)
    .title(text(t!("device.actions.title")))
    .into()
}

fn device_report(svalin_report: &persistent::Report) -> Element<'_, Message> {
    let report = &svalin_report.system_report;
    card(
        column![
            container(
                fact_list()
                    .entry(
                        text(t!("device.report.agent-version")),
                        svalin_report.current_version_identifier.as_str(),
                    )
                    .entry(
                        text(t!("device.report.hostname")),
                        report.hostname.as_deref().unwrap_or_default()
                    )
                    .entry(
                        text(t!("device.report.os")),
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
                    .entry(
                        text(t!("device.report.kernel-version")),
                        report.kernel_version.as_str()
                    )
                    // .entry("CPU Brand:", report.cpu.brand.as_str())
                    .entry(
                        text(t!("device.report.cpu-model")),
                        report.cpu.model.as_str()
                    )
                    // .entry("CPU Architecture:", report.cpu.arch.as_str())
                    .entry(
                        if report.cpu.cores.is_some() {
                            text(t!("device.report.cpu-cores-threads"))
                        } else {
                            text(t!("device.report.cpu-threads"))
                        },
                        if let Some(cores) = report.cpu.cores {
                            text!("{} / {}", cores, report.cpu.threads)
                        } else {
                            text!("{}", report.cpu.threads)
                        }
                    )
                    .entry(
                        text(t!("device.report.total-memory-swap")),
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
    .title(text(t!("device.report.title")))
    .action(
        icon_button(bootstrap::arrow_clockwise())
            .tooltip("Generate New Report")
            .on_press(Message::RefreshReport),
    )
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
                progress_bar(
                    0.0..=disk.total_space as f32,
                    (disk.total_space - disk.available_space) as f32
                )
                .style(progress_bar::secondary)
                .girth(Length::Fill),
                row![
                    text(t!(
                        "device.report.disk-free",
                        "free" => human_i_bytes(disk.available_space),
                        "total" => human_i_bytes(disk.total_space)
                    ))
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
