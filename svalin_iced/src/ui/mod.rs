use components::text_grid::AnsiGrid;
use iced::{
    keyboard::{self, key::Named},
    widget::{focus_next, stack},
    Subscription, Task,
};
use mainview::MainView;
use profile_picker::ProfilePicker;
use screen::SubScreen;

use crate::Element;

pub mod components;
pub mod mainview;
mod profile_picker;
pub mod screen;
pub mod types;
pub mod widgets;

pub enum Screen {
    ProfilePicker(ProfilePicker),
    MainView(MainView),
}

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    Tab,
    ProfilePicker(profile_picker::Message),
    MainView(mainview::Message),
}

pub struct UI {
    screen: Screen,
    test: AnsiGrid,
}

impl UI {
    pub fn start() -> (Self, Task<Message>) {
        let (screen, task) = ProfilePicker::start();
        let mut test = AnsiGrid::new(80, 25);
        test.parse(&include_str!("test")).unwrap();

        (
            Self {
                screen: Screen::ProfilePicker(screen),
                test: test,
            },
            task.map(Message::ProfilePicker),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tab => focus_next(),
            Message::ProfilePicker(profile_picker::Message::Profile(client)) => {
                let (state, task) = MainView::start(client);

                self.screen = Screen::MainView(state);
                task.map(Into::into)
            }
            Message::ProfilePicker(message) => match &mut self.screen {
                Screen::ProfilePicker(profile_picker) => {
                    profile_picker.update(message).map(Into::into)
                }
                _ => Task::none(),
            },
            Message::MainView(message) => match &mut self.screen {
                Screen::MainView(main_view) => main_view.update(message).map(Into::into),
                _ => Task::none(),
            },
        }
    }

    pub fn view(&self) -> Element<Message> {
        return self.test.view();

        let screen: Element<Message> = match &self.screen {
            Screen::ProfilePicker(profile_picker) => profile_picker.view().map(Into::into),
            Screen::MainView(mainview) => mainview.view().map(Into::into),
        };

        let dialog = match &self.screen {
            Screen::ProfilePicker(profile_picker) => profile_picker
                .dialog()
                .map(|element| element.map(Into::into)),
            Screen::MainView(mainview) => mainview.dialog().map(|element| element.map(Into::into)),
        };

        stack![screen].push_maybe(dialog).into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![keyboard::on_key_press(|key, _modifiers| match key {
            keyboard::Key::Named(named) => match named {
                Named::Tab => Some(Message::Tab),
                _ => None,
            },
            keyboard::Key::Character(_) => None,
            keyboard::Key::Unidentified => None,
        })];

        match &self.screen {
            Screen::ProfilePicker(_profile_picker) => (),
            Screen::MainView(mainview) => {
                subscriptions.push(mainview.subscription().map(Into::into));
            }
        };

        Subscription::batch(subscriptions)
    }
}
