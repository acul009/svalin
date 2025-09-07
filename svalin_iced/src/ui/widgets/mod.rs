use std::{borrow::Cow, ops::RangeInclusive};

use svalin::client::device::RemoteData;
use svalin_sysctl::realtime::RealtimeStatus;

use crate::Element;

pub mod card;
pub mod dialog;
pub mod error_display;
pub mod form;
pub mod header;
pub mod icon;
pub mod loading;
pub mod percent_display;
pub mod progress_circle;
pub mod realtime;
pub mod scaffold;

pub fn card<'a, Message>(
    title: impl Into<Element<'a, Message>>,
    content: impl Into<Element<'a, Message>>,
) -> card::Card<'a, Message> {
    card::Card::new(title, content)
}

pub fn form<'a, Message>() -> form::Form<'a, Message> {
    form::Form::new()
}

pub fn error_display<Error, Message>(
    error: &Error,
) -> error_display::ErrorDisplay<'_, Error, Message> {
    error_display::ErrorDisplay::new(error)
}

pub fn loading<'a>(message: impl Into<Cow<'a, str>>) -> loading::Loading<'a> {
    loading::Loading::new(message)
}

pub fn dialog<'a, Message>() -> dialog::Dialog<'a, Message> {
    dialog::Dialog::new()
}

pub fn header<'a, Message>(
    content: impl Into<Element<'a, Message>>,
) -> header::Header<'a, Message> {
    header::Header::new(content)
}

pub fn realtime(realtime: &RemoteData<RealtimeStatus>) -> realtime::RealtimeDisplay<'_> {
    realtime::RealtimeDisplay::new(realtime)
}

pub fn progress_circle<'a, Theme>(
    range: RangeInclusive<f32>,
    value: f32,
) -> progress_circle::ProgressCircle<Theme>
where
    Theme: progress_circle::StyleSheet + 'a,
{
    progress_circle::ProgressCircle::new(range, value)
}

pub fn percent_display<'a, Message>(
    range: RangeInclusive<f32>,
    value: f32,
) -> percent_display::PercentDisplay<'a, Message> {
    percent_display::PercentDisplay::new(range, value)
}

pub fn scaffold<'a, Message>(
    content: impl Into<Element<'a, Message>>,
) -> scaffold::Scaffold<'a, Message> {
    scaffold::Scaffold::new(content)
}
