use iced::widget::{column, container, progress_bar, row, text};
use svalin::client::device::RemoteLiveData;
use svalin_sysctl::realtime::RealtimeStatus;

use crate::Element;

use super::progress_circle;

pub struct RealtimeDisplay<'a> {
    realtime: &'a RemoteLiveData<RealtimeStatus>,
}

impl<'a> RealtimeDisplay<'a> {
    pub fn new(realtime: &'a RemoteLiveData<RealtimeStatus>) -> Self {
        Self { realtime }
    }
}

impl<'a, Message: Clone + 'static> From<RealtimeDisplay<'a>> for Element<'a, Message> {
    fn from(value: RealtimeDisplay) -> Self {
        let content: Element<Message> = match value.realtime {
            RemoteLiveData::Unavailable => text("unavailable").into(),
            RemoteLiveData::Pending => text("pending").into(),
            RemoteLiveData::Ready(realtime) => column![
                container(
                    row(realtime
                        .cpu
                        .cores
                        .iter()
                        .enumerate()
                        .map(|(index, usage)| column![
                            progress_circle(0.0..=100.0, usage.load),
                            text!("{:.0}%", usage.load),
                        ]
                        .into()))
                    .wrap()
                ),
                container(column![
                    row![
                        text("mem"),
                        text!("{}/{}", realtime.memory.used, realtime.memory.total)
                    ],
                    row![
                        text("swap"),
                        text!("{}/{}", realtime.swap.used, realtime.swap.total)
                    ],
                ])
            ]
            .into(),
        };

        container(content).into()
    }
}
