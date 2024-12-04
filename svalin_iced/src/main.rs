use iced::application;
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

    iced::application(Title, UI::update, UI::view)
        .subscription(UI::subscription)
        .theme(|_| iced::Theme::Dark)
        .antialiasing(true)
        .centered()
        .run_with(UI::start)
        .unwrap();
}

struct Title;

impl application::Title<UI> for Title {
    fn title(&self, _state: &UI) -> String {
        t!("app-title").to_string()
    }
}
