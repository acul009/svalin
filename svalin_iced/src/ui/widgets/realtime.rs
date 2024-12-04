use iced::{
    widget::{column, container, row, text},
    Length, Padding,
};
use iced_aw::card;
use svalin::client::device::RemoteLiveData;
use svalin_sysctl::realtime::RealtimeStatus;

use crate::Element;

use super::{loading, percent_display};

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
            RemoteLiveData::Unavailable => text(t!("realtime.live-unavailable"))
                .width(Length::Fill)
                .center()
                .into(),
            RemoteLiveData::Pending => loading(t!("realtime.connecting")).into(),
            RemoteLiveData::Ready(realtime) => column![
                card(
                    text(t!("realtime.cpu")),
                    row(realtime.cpu.cores.iter().enumerate().map(|(index, usage)| {
                        let core = index + 1;
                        percent_display(0.0..=100.0, usage.load)
                            .label(t!("realtime.core", "id" => core))
                            .padding(Padding::new(5.0).left(10.0).right(10.0))
                            .into()
                    }))
                    .wrap(),
                )
                .padding(10.into()),
                card(
                    text(t!("realtime.memory")),
                    column![percent_display(
                        0.0..=(realtime.memory.total as f32),
                        realtime.memory.used as f32
                    )
                    .label(t!("realtime.ram"))
                    .subinfo(text!(
                        "{} / {} M",
                        realtime.memory.used / 1024 / 1024,
                        realtime.memory.total / 1024 / 1024
                    ))
                    .bar()]
                    .push_maybe(if realtime.swap.total > 0 {
                        Some(
                            percent_display(
                                0.0..=(realtime.swap.total as f32),
                                realtime.swap.used as f32,
                            )
                            .label(t!("realtime.swap"))
                            .subinfo(text!(
                                "{} / {} M",
                                realtime.swap.used / 1024 / 1024,
                                realtime.swap.total / 1024 / 1024
                            ))
                            .bar(),
                        )
                    } else {
                        None
                    })
                    .padding(20)
                    .spacing(20)
                )
            ]
            .padding(30)
            .spacing(30)
            .into(),
        };

        container(content).into()
    }
}
