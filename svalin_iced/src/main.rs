use ui::UI;

mod i18n;
mod ui;

type Theme = iced::Theme;
type Element<'a, Message> = iced::Element<'a, Message, crate::Theme>;

fn main() {
    // Get the system's preferred languages.
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();

    // Enable localizations to be applied.
    i18n::init(&requested_languages);

    let title = fl!("app-title");

    iced::application("Fuck you", UI::update, UI::view)
        .theme(|_| iced::Theme::Dark)
        .antialiasing(true)
        .centered()
        .run()
        .unwrap();
}
