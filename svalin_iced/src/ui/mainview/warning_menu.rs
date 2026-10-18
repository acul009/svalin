use std::sync::Arc;

use anyhow::anyhow;
use iced::{
    Length, Widget,
    widget::{button, column, container, row, scrollable, text},
};
use svalin::client::state::{ClientState, warning};

use crate::{
    Element,
    ui::widgets::{device_label, toast},
    util::{format_timestamp, human_i_bytes},
};

use super::Message;

pub fn header<'a>() -> crate::ui::widgets::header::Header<'a, Message> {
    crate::ui::widgets::header(text(t!("warnings.title")).size(20)).drawer()
}

pub fn view(state: &ClientState) -> Element<'_, Message> {
    let mut warnings = state.warnings().warnings().peekable();
    if warnings.peek().is_none() {
        return container(text(t!("warnings.empty"))).padding(20).boxed();
    }

    scrollable(
        column(warnings.map(|warning| match warning {
            warning::Warning::Device(spki_hash, warnings) => {
                let header = row![
                    device_label(state, spki_hash)
                        .text_size(24)
                        .spacing(10.0)
                        .width(Length::Fill)
                        .into_widget(),
                    button(text(t!("warnings.device.action")))
                        .on_press(Message::SelectDevice(spki_hash.clone())),
                ]
                .align_y(iced::Alignment::Center)
                .spacing(10);

                toast(
                    toast::Kind::Neutral,
                    column![
                        header,
                        column(warnings.iter().map(device_warning)).spacing(10)
                    ]
                    .width(Length::Fill)
                    .spacing(10),
                )
                .into_widget()
            }
        }))
        .padding(20)
        .spacing(20),
    )
    .boxed()
}

fn device_warning(warning: &warning::Device) -> Element<'_, Message> {
    let kind = match warning.severity() {
        warning::Severity::High => toast::Kind::Error,
        warning::Severity::Medium => toast::Kind::Warning,
        warning::Severity::Low => toast::Kind::Info,
    };
    let details = if let warning::Device::DeviceGroupBroken(reason) = warning {
        Some(
            button(text(t!("generic.details")))
                .on_press(Message::Error(Arc::new(anyhow!(reason.to_owned()))))
                .boxed(),
        )
    } else {
        None
    };

    let message = match warning {
        warning::Device::DeviceGroupBroken(_) => text(t!("warnings.device.group-broken")),
        warning::Device::DiskSpaceLow { disk, free, total } => text(t!(
            "warnings.device.disk-space-low",
            "disk" => disk,
            "free" => human_i_bytes(*free),
            "total" => human_i_bytes(*total)
        )),
        warning::Device::MissingName => text(t!("warnings.device.missing-name")),
        warning::Device::BitlockerActive { drive, .. } => {
            text(t!("warnings.device.bitlocker-active", "drive" => drive))
        }
        warning::Device::PMGAttachmentQuarantine(count) => {
            text(t!("warnings.device.attachment-quarantine", "count" => count))
        }
        warning::Device::PMGVirusQuarantine(count) => {
            text(t!("warnings.device.virus-quarantine", "count" => count))
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
    toast(
        kind,
        column([message.boxed()].into_iter().chain(details))
            .width(Length::Fill)
            .spacing(10),
    )
    .into_widget()
    .boxed()
}
