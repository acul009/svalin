use iced::{
    Alignment::Center,
    Length,
    widget::{button, container, row, scrollable, stack, text},
};
use svalin::client::state::{ClientState, persistent};
use svalin_pki::SpkiHash;

use crate::{
    bootstrap,
    ui::widgets::{button_list, device_icon},
    util::format_timestamp,
};

pub struct DeviceList<'a, Message> {
    state: &'a ClientState,
    on_select: Option<Box<dyn Fn(SpkiHash) -> Message + 'a>>,
    on_new: Option<Message>,
    group_filter: &'a super::GroupFilter,
    search: &'a str,
}

impl<'a, Message> DeviceList<'a, Message> {
    pub fn new(state: &'a ClientState) -> Self {
        Self {
            state,
            on_select: None,
            on_new: None,
            group_filter: &super::GroupFilter::NoFilter,
            search: "",
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

    pub(crate) fn group_filter(mut self, group_filter: &'a super::GroupFilter) -> Self {
        self.group_filter = group_filter;
        self
    }

    pub(crate) fn search(mut self, search: &'a str) -> Self {
        self.search = search;
        self
    }
}

impl<'a, Message: Clone + 'static> DeviceList<'a, Message> {
    pub fn into_widget(self) -> impl iced::Widget<Message> + 'a {
        let device_list = self;
        let words: Vec<_> = device_list
            .search
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let mut devices: Vec<_> = match device_list.group_filter {
            super::GroupFilter::NoFilter => device_list
                .state
                .persistent()
                .devices()
                .iter()
                .filter(|(_, device)| matches_search(device, &words))
                .collect(),
            super::GroupFilter::Group(group) => device_list
                .state
                .group(group)
                .filter(|(_, device)| matches_search(device, &words))
                .collect(),
        };
        devices.sort_by_cached_key(|(spki_hash, device)| {
            (device.name().to_lowercase(), (*spki_hash).clone())
        });
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
            scrollable(
                button_list(devices.into_iter().map(|(spki_hash, persistent)| {
                    list_entry(
                        device_list.on_select.as_ref().map(|f| f(spki_hash.clone())),
                        persistent,
                        device_list.state.agent_online(spki_hash),
                    )
                }))
                .into_widget()
            )
        ]
    }
}

fn matches_search(device: &persistent::DeviceState, words: &[String]) -> bool {
    if words.is_empty() {
        return true;
    }
    let name = device.name().to_lowercase();
    words.iter().all(|word| name.contains(word))
}

fn list_entry<'a, Message: 'a>(
    on_press_maybe: Option<Message>,
    persistent: &'a persistent::DeviceState,
    is_online: bool,
) -> button_list::Entry<'a, Message> {
    button_list::entry(
        row![
            device_icon(&persistent.os(), is_online).size(30),
            text(persistent.name()),
            text(
                persistent
                    .report()
                    .map(|report| { format_timestamp(report.system_report.generated_at) })
                    .unwrap_or_else(|| t!("generic.unknown").to_string())
            )
        ]
        .spacing(20)
        .align_y(Center)
        .width(Length::Fill),
    )
    .on_press_maybe(on_press_maybe)
}
