use std::borrow::Cow;

use iced::{
    Length, Pixels,
    widget::{column, container, text},
};

use crate::Element;

pub struct Card<'a, Message> {
    title: Element<'a, Message>,
    content: Element<'a, Message>,
    padding: Pixels,
}

impl<'a, Message> Card<'a, Message> {
    pub fn new(
        title: impl Into<Element<'a, Message>>,
        content: impl Into<Element<'a, Message>>,
    ) -> Self {
        Self {
            title: title.into(),
            content: content.into(),
            padding: Pixels(16.0),
        }
    }

    pub fn padding(mut self, padding: Pixels) -> Self {
        self.padding = padding;
        self
    }
}

impl<'a, Message> From<Card<'a, Message>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(card: Card<'a, Message>) -> Self {
        container(column![
            container(card.title)
                .style(container::primary)
                .width(Length::Fill)
                .padding(card.padding),
            container(card.content).padding(card.padding)
        ])
        .style(container::bordered_box)
        .clip(true)
        .into()
    }
}
