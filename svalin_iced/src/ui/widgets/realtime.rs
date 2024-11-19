use iced::{
    alignment,
    widget::{column, container, progress_bar, row, text},
    Padding,
};
use iced_aw::{card, direction::Horizontal};
use svalin::client::device::RemoteLiveData;
use svalin_sysctl::realtime::RealtimeStatus;

use crate::{fl, Element};

use super::{loading, percent_display, progress_circle};

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
            RemoteLiveData::Pending => loading(fl!("connecting")).into(),
            RemoteLiveData::Ready(realtime) => column![
                card(
                    text(fl!("cpu")),
                    row(realtime.cpu.cores.iter().enumerate().map(|(index, usage)| {
                        let core = index + 1;
                        percent_display(0.0..=100.0, usage.load)
                            .label(fl!("core-id", core = core.to_string()))
                            .padding(Padding::new(5.0).left(10.0).right(10.0))
                            .into()
                    }))
                    .wrap(),
                )
                .padding(10.into()),
                card(
                    text(fl!("memory")),
                    column![percent_display(
                        0.0..=(realtime.memory.total as f32),
                        realtime.memory.used as f32
                    )
                    .label(fl!("ram"))
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
                            .label(fl!("swap"))
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
