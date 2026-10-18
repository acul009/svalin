use std::borrow::Cow;

use iced::{
    Length, Pixels, Widget,
    alignment::Vertical,
    widget::{row, text},
};
use svalin::client::state::ClientState;
use svalin_pki::SpkiHash;
use svalin_sysctl::system_report::OSFamily;

use super::device_icon;

pub struct DeviceLabel<'a> {
    state: &'a ClientState,
    spki_hash: &'a SpkiHash,
    text_size: Option<Pixels>,
    spacing: f32,
    width: Length,
}

impl<'a> DeviceLabel<'a> {
    pub fn new(state: &'a ClientState, spki_hash: &'a SpkiHash) -> Self {
        Self {
            state,
            spki_hash,
            text_size: None,
            spacing: 20.0,
            width: Length::Shrink,
        }
    }

    pub fn text_size(mut self, size: impl Into<Pixels>) -> Self {
        self.text_size = Some(size.into());
        self
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    pub fn into_widget<Message: 'static>(self) -> impl Widget<Message> + 'a {
        let device = self.state.persistent().devices().get(self.spki_hash);
        let name = device
            .map(|device| device.name())
            .unwrap_or_else(|| Cow::Owned(self.spki_hash.to_string()));
        let os = device
            .map(|device| device.os())
            .unwrap_or(OSFamily::Unknown);
        let name = text(name).width(self.width);
        let name = if let Some(size) = self.text_size {
            name.size(size)
        } else {
            name
        };

        row![
            device_icon(&os, self.state.agent_online(self.spki_hash)).size(30),
            name,
        ]
        .align_y(Vertical::Center)
        .spacing(self.spacing)
        .width(self.width)
    }
}
