use std::{borrow::Cow, sync::Arc};

use anyhow::anyhow;
use iced::{
    Length,
    widget::{button, column, container, row, scrollable, text},
};
use svalin::client::state::{ClientState, warning};

use crate::{
    Element,
    ui::widgets::toast,
    util::{format_timestamp, human_i_bytes},
};

use super::Message;

pub fn header<'a>() -> crate::ui::widgets::header::Header<'a, Message> {
    crate::ui::widgets::header(text(t!("warnings.title")).size(20)).drawer()
}

pub fn view(state: &ClientState) -> Element<'_, Message> {
    let mut warnings = state.warnings().warnings().peekable();
    if warnings.peek().is_none() {
        return container(text(t!("warnings.empty"))).padding(20).into();
    }

    scrollable(
        column(warnings.map(|warning| {
            Element::from(toast(
                match warning.severity() {
                    warning::Severity::High => toast::Kind::Error,
                    warning::Severity::Medium => toast::Kind::Warning,
                    warning::Severity::Low => toast::Kind::Info,
                },
                match warning {
                    warning::Warning::Device(spki_hash, warning) => {
                        let name = state
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
                                actions =
                                    actions.push(button(text(t!("generic.details"))).on_press(
                                        Message::Error(Arc::new(anyhow!(reason.to_owned()))),
                                    ));
                                text(t!("warnings.device.group-broken"))
                            }
                            warning::Device::DiskSpaceLow { disk, free, total } => text(t!(
                                "warnings.device.disk-space-low",
                                "disk" => disk,
                                "free" => human_i_bytes(*free),
                                "total" => human_i_bytes(*total)
                            )),
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
                            warning::Device::BackupOverdue { name, due_at } => text(t!(
                                "warnings.device.backup-overdue",
                                "name" => name,
                                "due_at" => format_timestamp(*due_at)
                            )),
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
                                    "finished_at" => format_timestamp(*finished_at)
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
    .into()
}
