use std::{borrow::Cow, ops::RangeInclusive};

// use svalin::client::device::RemoteData;
// use svalin_sysctl::realtime::RealtimeStatus;

use iced::{
    alignment::Vertical,
    color,
    widget::{Button, Text, button, row, text},
};
use svalin_sysctl::sytem_report::OSFamily;

use crate::{Element, bootstrap};

pub mod card;
pub mod dialog;
pub mod error_display;
pub mod fact_list;
pub mod form;
pub mod header;
pub mod icon_button;
pub mod list;
pub mod loading;
pub mod percent_display;
pub mod progress_circle;
pub mod scaffold;
pub mod toast;

pub fn device_icon(family: &OSFamily, online: bool) -> iced::widget::Text<'static> {
    os_icon(family).color(if online {
        color!(0x1fd11f)
    } else {
        color!(0xd60000)
    })
}

pub fn os_icon(family: &OSFamily) -> iced::widget::Text<'static> {
    match family {
        OSFamily::Windows => bootstrap::windows(),
        OSFamily::Linux => bootstrap::tux(),
        OSFamily::Unknown => bootstrap::laptop(),
    }
    .size(24)
    .center()
}

pub fn card<'a, Message>(content: impl Into<Element<'a, Message>>) -> card::Card<'a, Message> {
    card::Card::new(content)
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

pub fn dialog<'a, Message>(body: impl Into<Element<'a, Message>>) -> dialog::Dialog<'a, Message> {
    dialog::Dialog::new(body)
}

pub fn header<'a, Message: 'static>(
    content: impl Into<Element<'a, Message>>,
) -> header::Header<'a, Message> {
    header::Header::new(content)
}

pub fn list<'a, Message>(
    children: impl IntoIterator<Item = Element<'a, Message>>,
) -> list::List<'a, Message> {
    list::List::with_children(children)
}

pub fn fact_list<'a, Message>() -> fact_list::FactList<'a, Message> {
    fact_list::FactList::new()
}

pub fn icon_button<'a, Message>(icon: Text<'a>) -> icon_button::IconButton<'a, Message> {
    icon_button::IconButton::new(icon)
}

pub fn back_button<'a, Message: 'static>() -> Button<'a, Message> {
    button(
        row![bootstrap::arrow_left(), text(t!("generic.back"))]
            .align_y(Vertical::Center)
            .spacing(10),
    )
}

pub fn forward_button<'a, Message: 'static>() -> Button<'a, Message> {
    button(
        row![text(t!("generic.continue")), bootstrap::arrow_right()]
            .align_y(Vertical::Center)
            .spacing(10),
    )
}

// pub fn realtime(realtime: &RemoteData<RealtimeStatus>) -> realtime::RealtimeDisplay<'_> {
//     realtime::RealtimeDisplay::new(realtime)
// }

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

pub fn toast<'a, Message>(
    kind: toast::Kind,
    content: impl Into<Element<'a, Message>>,
) -> toast::Toast<'a, Message> {
    toast::Toast::new(kind, content)
}
