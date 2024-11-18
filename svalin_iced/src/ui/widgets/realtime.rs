use iced::{
    alignment,
    widget::{column, container, progress_bar, row, text},
    Padding,
};
use iced_aw::{card, direction::Horizontal};
use svalin::client::device::RemoteLiveData;
use svalin_sysctl::realtime::RealtimeStatus;

use crate::{fl, Element};

use super::{percent_display, progress_circle};

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
                card(
                    text(fl!("cpu")),
                    row(realtime.cpu.cores.iter().enumerate().map(|(index, usage)| {
                        percent_display(0.0..=100.0, usage.load)
                            .size(50.0)
                            .padding(Padding::new(5.0).left(10.0).right(10.0))
                            .into()
                    }))
                    .wrap(),
                )
                .padding(10.into()),
                card(
                    text(fl!("memory")),
                    column![
                        row![
                            text("mem"),
                            text!("{}/{}", realtime.memory.used, realtime.memory.total)
                        ],
                        row![
                            text("swap"),
                            text!("{}/{}", realtime.swap.used, realtime.swap.total)
                        ],
                    ]
                )
            ]
            .padding(30)
            .spacing(30)
            .into(),
        };

        container(content).into()
    }
}
