use iced::{
    Length, Padding,
    widget::{column, container, row, text},
};
use svalin::client::device::RemoteData;
use svalin_sysctl::realtime::RealtimeStatus;

use crate::{Element, ui::widgets::card};

use super::{loading, percent_display};

pub struct RealtimeDisplay<'a> {
    realtime: &'a RemoteData<RealtimeStatus>,
}

impl<'a> RealtimeDisplay<'a> {
    pub fn new(realtime: &'a RemoteData<RealtimeStatus>) -> Self {
        Self { realtime }
    }
}

impl<'a, Message: Clone + 'static> From<RealtimeDisplay<'a>> for Element<'a, Message> {
    fn from(value: RealtimeDisplay) -> Self {
        let content: Element<Message> = match value.realtime {
            RemoteData::Unavailable => container(text(t!("realtime.live-unavailable")).center())
                .padding(20)
                .width(Length::Fill)
                .into(),
            RemoteData::Pending => container(loading(t!("realtime.connecting")))
                .height(200)
                .into(),
            RemoteData::Ready(realtime) => column![
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
                    column![
                        percent_display(
                            0.0..=(realtime.memory.total as f32),
                            realtime.memory.used as f32
                        )
                        .label(t!("realtime.ram"))
                        .subinfo(text!(
                            "{} / {} M",
                            realtime.memory.used / 1024 / 1024,
                            realtime.memory.total / 1024 / 1024
                        ))
                        .bar(),
                        if realtime.swap.total > 0 {
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
                        }
                    ]
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
