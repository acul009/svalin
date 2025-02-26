use ui::UI;

pub mod ui;
pub mod util;

#[macro_use]
extern crate rust_i18n;
i18n!("locales", fallback = "en");

type Theme = iced::Theme;
type Element<'a, Message> = iced::Element<'a, Message, crate::Theme>;

fn main() {
    svalin::tracing_subscriber::fmt::init();

    iced::daemon(UI::title, UI::update, UI::view)
        .subscription(UI::subscription)
        .theme(|_, _| iced::Theme::Dark)
        .antialiasing(true)
        .run_with(UI::start)
        .unwrap();
}
