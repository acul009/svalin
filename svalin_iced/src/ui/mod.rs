use iced::{
    keyboard::{self, key::Named},
    widget::{center, focus_next, text},
    window, Subscription, Task,
};
use mainview::MainView;
use profile_picker::ProfilePicker;
use screen::SubScreen;
use widgets::scaffold;

use crate::Element;

pub mod components;
mod mainview;
mod profile_picker;
pub mod screen;
pub mod types;
pub mod widgets;
mod window_helper;

pub enum Screen {
    ProfilePicker(ProfilePicker),
    MainView(MainView),
}

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    Noop,
    Tab,
    ProfilePicker(profile_picker::Message),
    MainView(mainview::Message),
}

pub struct UI {
    screen: Screen,
    main_window_id: window::Id,
}

impl UI {
    pub fn start() -> (Self, Task<Message>) {
        let (id, open) = window::open(window::Settings::default());

        let (screen, task) = ProfilePicker::start();

        let task = Task::batch(vec![
            open.map(|_| Message::Noop),
            task.map(Message::ProfilePicker),
        ]);

        (
            Self {
                screen: Screen::ProfilePicker(screen),
                main_window_id: id,
            },
            task,
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Noop => Task::none(),
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

    pub fn title(&self, _window_id: window::Id) -> String {
        t!("app-title").to_string()
    }

    pub fn view(&self, window_id: window::Id) -> Element<Message> {
        if window_id == self.main_window_id {
            let screen: Element<Message> = match &self.screen {
                Screen::ProfilePicker(profile_picker) => profile_picker.view().map(Into::into),
                Screen::MainView(mainview) => mainview.view().map(Into::into),
            };

            let header = match &self.screen {
                Screen::ProfilePicker(profile_picker) => profile_picker.header().mapopt(Into::into),
                Screen::MainView(mainview) => mainview.header().mapopt(Into::into),
            };

            let dialog = match &self.screen {
                Screen::ProfilePicker(profile_picker) => profile_picker.dialog().mapopt(Into::into),
                Screen::MainView(mainview) => mainview.dialog().mapopt(Into::into),
            };

            let context = match &self.screen {
                Screen::ProfilePicker(profile_picker) => {
                    profile_picker.context().mapopt(Into::into)
                }
                Screen::MainView(mainview) => mainview.context().mapopt(Into::into),
            };

            scaffold(screen)
                .header_maybe(header)
                .dialog_maybe(dialog)
                .context_maybe(context)
                .into()
        } else {
            center(text("TODO")).into()
        }
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

pub trait MapOpt<'a, From, To> {
    fn mapopt<F>(self, f: F) -> Option<Element<'a, To>>
    where
        F: Fn(From) -> To,
        F: 'a;
}

impl<'a, From, To> MapOpt<'a, From, To> for Option<Element<'a, From>>
where
    From: 'a,
    To: 'a,
{
    fn mapopt<F>(self, f: F) -> Option<Element<'a, To>>
    where
        F: Fn(From) -> To,
        F: 'a,
    {
        self.map(|element| element.map(f))
    }
}
