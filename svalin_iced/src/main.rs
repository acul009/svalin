use ui::UI;

mod i18n;
mod ui;

fn main() {
    let title = fl!("app-title");

    iced::application("Fuck you", UI::update, UI::view)
        .theme(|_| iced::Theme::Dark)
        .antialiasing(true)
        .centered()
        .run()
        .unwrap();
}
