use iced::{Subscription, Task, keyboard, window};
use mainview::MainView;
use profile_picker::ProfilePicker;
use widgets::scaffold;
// use window_helper::WindowHelper;

use crate::Element;

pub mod components;
mod mainview;
mod profile_picker;
pub mod types;
pub mod widgets;
// mod window_helper;

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
    // WindowHelper(window_helper::Message),
    WindowCloseRequest(window::Id),
}

pub struct UI {
    screen: Screen,
    main_window_id: window::Id,
    // window_helper: WindowHelper,
}

impl UI {
    pub fn start() -> (Self, Task<Message>) {
        let (id, open) = window::open(window::Settings {
            exit_on_close_request: false,
            ..Default::default()
        });

        let (screen, task) = ProfilePicker::start();

        let task = Task::batch(vec![open.discard(), task.map(Message::ProfilePicker)]);

        (
            Self {
                screen: Screen::ProfilePicker(screen),
                main_window_id: id,
                // window_helper: WindowHelper::new(),
            },
            task,
        )
    }

    #[must_use]
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tab => iced::widget::operation::focus_next(),
            Message::WindowCloseRequest(id) => {
                if id == self.main_window_id {
                    if let Screen::MainView(main) = &mut self.screen {
                        return main.shutdown().map(Message::MainView);
                    }
                    // Todo: proper shutdown
                    iced::exit()
                } else {
                    // self.window_helper.close_window(&id);
                    Task::none()
                }
            }
            Message::ProfilePicker(message) => match &mut self.screen {
                Screen::ProfilePicker(profile_picker) => {
                    let action = profile_picker.update(message);

                    match action {
                        profile_picker::Action::OpenProfile(client) => {
                            let (state, task) = MainView::new(client);

                            self.screen = Screen::MainView(state);

                            task.map(Message::MainView)
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
                        // mainview::Action::OpenTerminal(device) => self
                        //     .window_helper
                        //     .add_terminal(device)
                        //     .map(Message::WindowHelper),
                    }
                }
                _ => Task::none(),
            },
            // Message::WindowHelper(message) => self
            //     .window_helper
            //     .update(message)
            //     .map(Message::WindowHelper),
        }
    }

    pub fn title(&self, window_id: window::Id) -> String {
        if window_id == self.main_window_id {
            t!("app-title").to_string()
        } else {
            "TODO".to_string()
            // self.window_helper.title(window_id)
        }
    }

    pub fn view(&self, window_id: window::Id) -> Element<'_, Message> {
        if window_id == self.main_window_id {
            let screen: Element<Message> = match &self.screen {
                Screen::ProfilePicker(profile_picker) => {
                    profile_picker.view().map(Message::ProfilePicker)
                }
                Screen::MainView(mainview) => mainview.view().map(Message::MainView),
            };

            let header = match &self.screen {
                Screen::ProfilePicker(_) => iced::widget::space().into(),
                Screen::MainView(mainview) => mainview.header().map(Message::MainView),
            };

            let context = match &self.screen {
                Screen::ProfilePicker(_) => None,
                Screen::MainView(mainview) => {
                    if let Some(context) = mainview.context() {
                        Some(context.map(Message::MainView))
                    } else {
                        None
                    }
                }
            };

            scaffold(screen)
                .header(header)
                .context_maybe(context)
                .into()
        } else {
            "Todo".into()
            // self.window_helper
            //     .view(window_id)
            //     .map(Message::WindowHelper)
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            keyboard::listen().filter_map(|event| {
                if let keyboard::Event::KeyPressed { key, .. } = event {
                    match key {
                        keyboard::Key::Named(keyboard::key::Named::Tab) => Some(Message::Tab),
                        _ => None,
                    }
                } else {
                    None
                }
            }),
            window::close_requests().map(Message::WindowCloseRequest),
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
