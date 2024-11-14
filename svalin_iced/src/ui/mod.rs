use iced::{
    alignment::Horizontal,
    widget::{column, stack, text},
    Length, Task,
};
use profile_picker::ProfilePicker;
use screen::SubScreen;

use crate::Element;

mod profile_picker;
pub mod screen;
pub mod types;
pub mod widgets;

pub enum Screen {
    ProfilePicker(ProfilePicker),
    Success,
}

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    ProfilePicker(profile_picker::Message),
}

pub struct UI {
    screen: Screen,
}

impl Default for UI {
    fn default() -> Self {
        Self {
            screen: Screen::ProfilePicker(ProfilePicker::new()),
        }
    }
}

impl UI {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ProfilePicker(message) => match message {
                profile_picker::Message::Profile(client) => self.screen = Screen::Success,
                _ => {
                    if let Screen::ProfilePicker(profile_picker) = &mut self.screen {
                        return profile_picker.update(message).map(Message::ProfilePicker);
                    }
                }
            },
        }
        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
        let screen: Element<Message> = match &self.screen {
            Screen::ProfilePicker(profile_picker) => profile_picker
                .view()
                .map(|msg| Message::ProfilePicker(msg))
                .into(),
            Screen::Success => column![text("success")
                .height(Length::Fill)
                .align_x(Horizontal::Center),]
            .into(),
        };

        let dialog = match &self.screen {
            Screen::ProfilePicker(profile_picker) => profile_picker
                .dialog()
                .map(|element| element.map(Message::ProfilePicker)),
            Screen::Success => None,
        };

        stack![screen].push_maybe(dialog).into()
    }
}
