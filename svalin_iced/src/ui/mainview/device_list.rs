use iced::{
    Alignment::Center,
    Color, Length,
    widget::{button, column, container, row, stack, text},
};
use svalin::client::state::ClientState;
use svalin_pki::SpkiHash;

use crate::{Element, ui::widgets::icon};

pub struct DeviceList<'a, Message> {
    state: &'a ClientState,
    on_select: Option<Box<dyn Fn(SpkiHash) -> Message + 'a>>,
    on_new: Option<Message>,
}

impl<'a, Message> DeviceList<'a, Message> {
    pub fn new(state: &'a ClientState) -> Self {
        Self {
            state,
            on_select: None,
            on_new: None,
        }
    }

    pub fn on_select(mut self, on_select: impl Fn(SpkiHash) -> Message + 'a) -> Self {
        self.on_select = Some(Box::new(on_select));
        self
    }

    pub fn on_new(mut self, on_new: Message) -> Self {
        self.on_new = Some(on_new);
        self
    }
}

impl<'a, Message: Clone + 'static> From<DeviceList<'a, Message>> for Element<'a, Message> {
    fn from(device_list: DeviceList<'a, Message>) -> Self {
        stack![
            container(
                button(
                    row![icon::add().size(30), text(t!("device_list.add"))]
                        .align_y(Center)
                        .spacing(10)
                        .padding([0, 10])
                )
                .on_press_maybe(device_list.on_new)
            )
            .align_bottom(Length::Fill)
            .align_right(Length::Fill)
            .padding(30),
            column(
                device_list
                    .state
                    .persistent()
                    .iter()
                    .map(|(spki_hash, persistent)| button(row![
                        icon::device().color(if device_list.state.agent_online(spki_hash) {
                            Color::from_rgb8(0, 255, 0)
                        } else {
                            Color::from_rgb8(255, 0, 0)
                        }),
                        text!("{}", spki_hash)
                    ])
                    .into()),
            )
        ]
        .into()
    }
}
