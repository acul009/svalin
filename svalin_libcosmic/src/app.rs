use crate::config::Config;
use crate::ui::profile_picker::ProfilePicker;
use crate::{fl, ui::profile_picker};
use cosmic::app::{Core, Task};
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::alignment::Horizontal;
use cosmic::iced::{Alignment, Length, Subscription};
use cosmic::iced_widget::{button, Column};
use cosmic::widget::{self, icon, menu, nav_bar, row, text};
use cosmic::{cosmic_theme, theme, Application, ApplicationExt, Apply, Element};
use futures_util::SinkExt;
use std::collections::HashMap;

const REPOSITORY: &str = "https://github.com/pop-os/cosmic-app-template";
const APP_ICON: &[u8] = include_bytes!("../res/icons/hicolor/scalable/apps/icon.svg");

pub enum Screen {
    ProfilePicker(ProfilePicker),
    Success,
}

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    ProfilePicker(profile_picker::Message),
}

/// The application model stores app-specific state used to describe its
/// interface and drive its logic.
pub struct AppModel {
    /// Application state which is managed by the COSMIC runtime.
    core: Core,
    // Configuration data that persists between application runs.
    config: Config,

    screen: Screen,
}

/// Create a COSMIC application from the app model
impl Application for AppModel {
    /// The async executor that will be used to run your application's commands.
    type Executor = cosmic::executor::Default;

    /// Data that your application receives to its init method.
    type Flags = ();

    /// Messages which the application and its widgets will emit.
    type Message = Message;

    /// Unique identifier in RDNN (reverse domain name notation) format.
    const APP_ID: &'static str = "de.it-woelfchen.svalin";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    /// Initializes the application with any given flags and startup commands.
    fn init(mut core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        // Construct the app model with the runtime's core.
        let mut app = AppModel {
            core,
            // Optional configuration file for an application.
            config: cosmic_config::Config::new(Self::APP_ID, Config::VERSION)
                .map(|context| match Config::get_entry(&context) {
                    Ok(config) => config,
                    Err((_errors, config)) => {
                        // for why in errors {
                        //     tracing::error!(%why, "error loading app config");
                        // }

                        config
                    }
                })
                .unwrap_or_default(),
            screen: Screen::ProfilePicker(ProfilePicker::new()),
        };

        let mut tasks = Vec::<Task<Self::Message>>::new();

        let window_title = fl!("app-title");
        if let Some(id) = app.core.main_window_id() {
            tasks.push(app.set_window_title(window_title, id));
        }

        (app, Task::none())
    }

    /// Elements to pack at the start of the header bar.
    fn header_start(&self) -> Vec<Element<Self::Message>> {
        let menu_bar = menu::bar(vec![]);

        vec![menu_bar.into()]
    }

    /// Enables the COSMIC application to create a nav bar with this model.
    fn nav_model(&self) -> Option<&nav_bar::Model> {
        None
    }

    /// Display a context drawer if the context page is requested.
    fn context_drawer(&self) -> Option<Element<Self::Message>> {
        if !self.core.window.show_context {
            return None;
        }

        match &self.screen {
            Screen::ProfilePicker(_profile_picker) => None,
            Screen::Success => None,
        }
    }

    /// Describes the interface based on the current state of the application
    /// model.
    ///
    /// Application events will be processed through the view. Any messages
    /// emitted by events received by widgets will be passed to the update
    /// method.
    fn view(&self) -> Element<Self::Message> {
        match &self.screen {
            Screen::ProfilePicker(profile_picker) => {
                return profile_picker.view().map(|msg| Message::ProfilePicker(msg))
            }
            Screen::Success => {
                return Column::new()
                    .push(text("success"))
                    .height(Length::Fill)
                    .align_x(Horizontal::Center)
                    .into()
            }
        }
    }

    fn dialog(&self) -> Option<Element<Self::Message>> {
        let dialog_element = match &self.screen {
            Screen::ProfilePicker(profile_picker) => profile_picker
                .dialog()
                .map(|element| element.map(|msg| Message::ProfilePicker(msg))),
            Screen::Success => None,
        };

        dialog_element
    }

    /// Register subscriptions for this application.
    ///
    /// Subscriptions are long-running async tasks running in the background
    /// which emit messages to the application through a channel. They are
    /// started at the beginning of the application, and persist through its
    /// lifetime.
    fn subscription(&self) -> Subscription<Self::Message> {
        Subscription::none()
    }

    /// Handles messages emitted by the application and its widgets.
    ///
    /// Tasks may be returned for asynchronous execution of code in the
    /// background on the application's async runtime.
    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::ProfilePicker(message) => {
                if let profile_picker::Message::Profile(client) = message {
                    self.screen = Screen::Success;
                    return Task::none();
                }

                if let Screen::ProfilePicker(profile_picker) = &mut self.screen {
                    let task = profile_picker
                        .update(message)
                        .map(|msg: profile_picker::Message| Message::ProfilePicker(msg));

                    return task.map(|msg| cosmic::app::Message::App(msg));
                }
            }
        }
        Task::none()
    }

    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<Self::Message> {
        Task::none()
    }
}
