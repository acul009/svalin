use iced::{
    alignment::Horizontal,
    widget::{column, text},
    Length, Task,
};
use profile_picker::ProfilePicker;

use crate::Element;

mod profile_picker;
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
                        profile_picker
                            .update(message)
                            .map(|msg: profile_picker::Message| Message::ProfilePicker(msg));
                    }
                }
            },
        }
        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
        match &self.screen {
            Screen::ProfilePicker(profile_picker) => profile_picker
                .view()
                .map(|msg| Message::ProfilePicker(msg))
                .into(),
            Screen::Success => column![text("success")
                .height(Length::Fill)
                .align_x(Horizontal::Center),]
            .into(),
        }
    }
}
