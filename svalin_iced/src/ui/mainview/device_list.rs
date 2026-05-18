use chrono::DateTime;
use iced::{
    Alignment::Center,
    Color, Length,
    widget::{button, column, container, row, stack, text},
};
// use iced_fonts::{
//     BOOTSTRAP_FONT,
//     bootstrap::{self, bootstrap},
// };
use svalin::client::state::ClientState;
use svalin_pki::SpkiHash;
use svalin_sysctl::sytem_report::OSFamily;

use crate::{Element, bootstrap};

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
                    row![bootstrap::plus().size(30), text(t!("device_list.add"))]
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
                    .map(|(spki_hash, persistent)| {
                        let color = if device_list.state.agent_online(spki_hash) {
                            Color::from_rgb8(0, 255, 0)
                        } else {
                            Color::from_rgb8(255, 0, 0)
                        };

                        button(
                            row![
                                match persistent.os() {
                                    OSFamily::Windows => bootstrap::windows(),
                                    OSFamily::Linux => bootstrap::tux(),
                                    OSFamily::Unknown => bootstrap::laptop(),
                                }
                                .size(16)
                                .color(color),
                                text!("{}", spki_hash),
                                text(
                                    persistent
                                        .report()
                                        .map(|report| {
                                            DateTime::from_timestamp_secs(
                                                report.system_report.generated_at as i64,
                                            )
                                        })
                                        .flatten()
                                        .map(|datetime| {
                                            datetime
                                                .naive_local()
                                                .format("%Y-%m-%d %H:%M:%S")
                                                .to_string()
                                        })
                                        .unwrap_or_else(|| "Unknown".to_string())
                                )
                            ]
                            .spacing(20)
                            .padding(10)
                            .width(Length::Fill),
                        )
                        .on_press_maybe(
                            device_list.on_select.as_ref().map(|f| f(spki_hash.clone())),
                        )
                        .style(button::subtle)
                        .into()
                    }),
            )
        ]
        .into()
    }
}
