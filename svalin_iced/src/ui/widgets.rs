use std::{borrow::Cow, ops::RangeInclusive};

// use svalin::client::device::RemoteData;
// use svalin_sysctl::realtime::RealtimeStatus;

use iced::{
    Widget,
    alignment::Vertical,
    color,
    widget::{Text, button, row, text},
};
use svalin_sysctl::system_report::OSFamily;

use crate::bootstrap;

pub mod button_list;
pub mod card;
pub mod device_label;
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
pub mod rail;
pub mod scaffold;
pub mod toast;

pub fn device_label<'a>(
    state: &'a svalin::client::state::ClientState,
    spki_hash: &'a svalin_pki::SpkiHash,
) -> device_label::DeviceLabel<'a> {
    device_label::DeviceLabel::new(state, spki_hash)
}

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

pub fn card<'a, Message, Content: Widget<Message> + 'a>(
    content: Content,
) -> card::Card<'a, Message, Content> {
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

pub fn dialog<'a, Message, Body: Widget<Message> + 'a>(
    body: Body,
) -> dialog::Dialog<'a, Message, Body> {
    dialog::Dialog::new(body)
}

pub fn header<'a, Message: 'static>(
    content: impl Widget<Message> + 'a,
) -> header::Header<'a, Message> {
    header::Header::new(content)
}

pub fn list<'a, Message>(
    children: impl IntoIterator<Item = impl Widget<Message> + 'a>,
) -> list::List<'a, Message> {
    list::List::with_children(children)
}

/// Creates a button list from entries.
pub fn button_list<'a, Message>(
    entries: impl IntoIterator<Item = button_list::Entry<'a, Message>>,
) -> button_list::ButtonList<'a, Message> {
    button_list::ButtonList::with_children(entries)
}

/// Creates a button list from entries built with `button_list::entry`.
#[macro_export]
macro_rules! button_list {
    ($($entry:expr),* $(,)?) => {
        $crate::ui::widgets::button_list([$($entry),*])
    };
}

pub use crate::button_list;

pub fn fact_list<'a, Message>() -> fact_list::FactList<'a, Message> {
    fact_list::FactList::new()
}

pub fn icon_button<'a, Message>(icon: Text<'a>) -> icon_button::IconButton<'a, Message> {
    icon_button::IconButton::new(icon)
}

pub fn back_button<'a, Message: Clone + 'static>() -> impl iced::Widget<Message> + 'a {
    button(
        row![bootstrap::arrow_left(), text(t!("generic.back"))]
            .align_y(Vertical::Center)
            .spacing(10),
    )
}

pub fn forward_button<'a, Message: Clone + 'static>() -> impl iced::Widget<Message> + 'a {
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

pub fn percent_display<'a>(
    range: RangeInclusive<f32>,
    value: f32,
) -> percent_display::PercentDisplay<'a> {
    percent_display::PercentDisplay::new(range, value)
}

pub fn scaffold<Body>(content: Body) -> scaffold::Scaffold<Body> {
    scaffold::Scaffold::new(content)
}

pub fn toast<Content>(kind: toast::Kind, content: Content) -> toast::Toast<Content> {
    toast::Toast::new(kind, content)
}
