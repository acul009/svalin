use iced::{
    widget::{self, button, row, text},
    Length,
};

use crate::Element;

pub struct Header<'a, Message> {
    content: Element<'a, Message>,
    on_back: Option<Message>,
}

impl<'a, Message> Header<'a, Message> {
    pub fn new(content: impl Into<Element<'a, Message>>) -> Self {
        Self {
            content: content.into(),
            on_back: None,
        }
    }

    pub fn on_back(mut self, message: Message) -> Self {
        self.on_back = Some(message);
        self
    }

    pub fn on_back_maybe(mut self, message: Option<Message>) -> Self {
        if let Some(message) = message {
            self.on_back = Some(message);
        }
        self
    }
}

impl<'a, Message: Clone + 'static> From<Header<'a, Message>> for Element<'a, Message> {
    fn from(header: Header<'a, Message>) -> Self {
        let mut row = match header.on_back {
            None => row!(),
            Some(on_back) => row![
                button(text("<").width(Length::Fill).height(Length::Fill).center())
                    .on_press(on_back)
                    .height(Length::Fill)
                    .width(40),
                widget::vertical_rule(2)
            ]
            .padding(5)
            .spacing(5),
        };

        row = row.push(header.content);

        row.into()
    }
}
