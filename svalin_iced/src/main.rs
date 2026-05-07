use ui::{UI, widgets::icon};

pub mod ui;
pub mod util;
use iced_fonts::generate_icon_functions;

pub const BOOTSTRAP_FONT_BYTES: &[u8] = include_bytes!("../fonts/bootstrap-icons.ttf");
pub const BOOTSTRAP_FONT: iced::Font = iced::Font::new("bootstrap-icons");
generate_icon_functions!(
    "svalin_iced/fonts/bootstrap-icons.ttf",
    bootstrap,
    BOOTSTRAP_FONT,
    "basic"
);

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
        .font(BOOTSTRAP_FONT_BYTES)
        .antialiasing(true)
        .run()
        .unwrap();
}
