use iced::{
    Length, Task,
    alignment::Horizontal,
    widget::{self, button, center, column, container, row, rule, scrollable, space, table, text},
};
use svalin::client::state::ClientState;
use svalin_pki::SpkiHash;

use crate::{
    Element, bootstrap,
    ui::widgets::{card, header, scaffold::HEADER_HEIGHT},
};

#[derive(Debug, Clone)]
pub enum Message {
    Back,
}

pub enum Action {
    None,
    Back,
    Run(Task<Message>),
}

pub struct DeviceView {
    spki_hash: SpkiHash,
}

impl DeviceView {
    pub fn new(spki_hash: SpkiHash) -> Self {
        Self { spki_hash }
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Back => Action::Back,
        }
    }

    pub fn view<'a>(&'a self, client_state: &'a ClientState) -> Element<'a, Message> {
        let Some(persistent) = client_state.persistent().get(&self.spki_hash) else {
            return center("Device not yet available").into();
        };

        scrollable(
            column![if let Some(report) = persistent.system_report() {
                Some(
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
                    .title("System Report"),
                )
            } else {
                None
            }]
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
