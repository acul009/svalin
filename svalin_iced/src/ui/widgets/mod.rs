use std::{borrow::Cow, ops::RangeInclusive};

use svalin::client::device::RemoteLiveData;
use svalin_sysctl::realtime::RealtimeStatus;

pub mod dialog;
pub mod error_display;
pub mod form;
pub mod loading;
pub mod percent_display;
pub mod progress_circle;
pub mod realtime;

pub fn form<'a, Message>() -> form::Form<'a, Message> {
    form::Form::new()
}

pub fn error_display<'a, Error, Message>(
    error: &'a Error,
) -> error_display::ErrorDisplay<'a, Error, Message> {
    error_display::ErrorDisplay::new(error)
}

pub fn loading<'a>(message: impl Into<Cow<'a, str>>) -> loading::Loading<'a> {
    loading::Loading::new(message)
}

pub fn dialog<'a, Message>() -> dialog::Dialog<'a, Message> {
    dialog::Dialog::new()
}

pub fn realtime<'a>(realtime: &'a RemoteLiveData<RealtimeStatus>) -> realtime::RealtimeDisplay<'a> {
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

pub fn percent_display<'a>(
    range: RangeInclusive<f32>,
    value: f32,
) -> percent_display::PercentDisplay<'a> {
    percent_display::PercentDisplay::new(range, value)
}
