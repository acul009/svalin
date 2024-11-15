use iced::application;
use ui::UI;

mod i18n;
mod ui;

type Theme = iced::Theme;
type Element<'a, Message> = iced::Element<'a, Message, crate::Theme>;

fn main() {
    svalin::tracing_subscriber::fmt::init();

    // Get the system's preferred languages.
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();

    // Enable localizations to be applied.
    i18n::init(&requested_languages);

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
        fl!("app-title")
    }
}
