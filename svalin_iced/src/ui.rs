use iced::{
    Subscription, Task,
    keyboard::{self, key::Named},
    widget::focus_next,
    window,
};
use mainview::MainView;
use profile_picker::ProfilePicker;
use widgets::scaffold;
use window_helper::WindowHelper;

use crate::Element;

pub mod components;
mod mainview;
mod profile_picker;
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
    WindowHelper(window_helper::Message),
}

pub struct UI {
    screen: Screen,
    main_window_id: window::Id,
    window_helper: WindowHelper,
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
                window_helper: WindowHelper::new(),
            },
            task,
        )
    }

    #[must_use]
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Noop => Task::none(),
            Message::Tab => focus_next(),
            Message::ProfilePicker(message) => match &mut self.screen {
                Screen::ProfilePicker(profile_picker) => {
                    let action = profile_picker.update(message);

                    match action {
                        profile_picker::Action::OpenProfile(client) => {
                            let state = MainView::new(client);

                            self.screen = Screen::MainView(state);

                            Task::none()
                        }
                        profile_picker::Action::None => Task::none(),
                        profile_picker::Action::Run(task) => task.map(Message::ProfilePicker),
                    }
                }
                _ => Task::none(),
            },
            Message::MainView(message) => match &mut self.screen {
                Screen::MainView(main_view) => {
                    let action = main_view.update(message);

                    match action {
                        mainview::Action::None => Task::none(),
                        mainview::Action::Run(task) => task.map(Message::MainView),
                        mainview::Action::OpenTerminal(device) => self
                            .window_helper
                            .add_terminal(device)
                            .map(Message::WindowHelper),
                    }
                }
                _ => Task::none(),
            },
            Message::WindowHelper(message) => self
                .window_helper
                .update(message)
                .map(Message::WindowHelper),
        }
    }

    pub fn title(&self, window_id: window::Id) -> String {
        if window_id == self.main_window_id {
            t!("app-title").to_string()
        } else {
            self.window_helper.title(window_id)
        }
    }

    pub fn view(&self, window_id: window::Id) -> Element<Message> {
        if window_id == self.main_window_id {
            let screen: Element<Message> = match &self.screen {
                Screen::ProfilePicker(profile_picker) => {
                    profile_picker.view().map(Message::ProfilePicker)
                }
                Screen::MainView(mainview) => mainview.view().map(Message::MainView),
            };

            let header = match &self.screen {
                Screen::ProfilePicker(_) => None,
                Screen::MainView(mainview) => mainview.header().mapopt(Message::MainView),
            };

            let dialog = match &self.screen {
                Screen::ProfilePicker(profile_picker) => {
                    profile_picker.dialog().mapopt(Message::ProfilePicker)
                }
                Screen::MainView(mainview) => mainview.dialog().mapopt(Message::MainView),
            };

            let context = match &self.screen {
                Screen::ProfilePicker(_) => None,
                Screen::MainView(mainview) => mainview.context().mapopt(Message::MainView),
            };

            scaffold(screen)
                .header_maybe(header)
                .dialog_maybe(dialog)
                .context_maybe(context)
                .into()
        } else {
            self.window_helper
                .view(window_id)
                .map(Message::WindowHelper)
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            keyboard::on_key_press(|key, _modifiers| match key {
                keyboard::Key::Named(named) => match named {
                    Named::Tab => Some(Message::Tab),
                    _ => None,
                },
                keyboard::Key::Character(_) => None,
                keyboard::Key::Unidentified => None,
            }),
            self.window_helper.subscription().map(Message::WindowHelper),
        ];

        match &self.screen {
            Screen::ProfilePicker(_profile_picker) => (),
            Screen::MainView(mainview) => {
                subscriptions.push(mainview.subscription().map(Message::MainView));
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
