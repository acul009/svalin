use ui::{UI, widgets::icon};

pub mod ui;
pub mod util;

#[macro_use]
extern crate rust_i18n;
i18n!("locales", fallback = "en");

type Theme = iced::Theme;
type Element<'a, Message> = iced::Element<'a, Message, crate::Theme>;

fn main() {
    svalin::tracing_subscriber::fmt::init();

    iced::daemon(UI::start, UI::update, UI::view)
        .title(UI::title)
        .subscription(UI::subscription)
        .theme(|_: &'_ UI, _| iced::Theme::Dark)
        .font(icon::FONT)
        .antialiasing(true)
        .run()
        .unwrap();
}
