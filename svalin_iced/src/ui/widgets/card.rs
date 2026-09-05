use iced::{
    Length, Padding,
    widget::{column, container},
};

use crate::Element;

pub struct Card<'a, Message> {
    title: Option<Element<'a, Message>>,
    content: Element<'a, Message>,
    padding: Padding,
}

impl<'a, Message> Card<'a, Message> {
    pub fn new(content: impl Into<Element<'a, Message>>) -> Self {
        Self {
            title: None,
            content: content.into(),
            padding: 16.into(),
        }
    }

    pub fn title(mut self, title: impl Into<Element<'a, Message>>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
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
                .style(|theme| container::Style {
                    background: Some(theme.palette().background.strong.color.into()),
                    ..Default::default()
                })
                .width(Length::Fill)
                .padding(16),
            container(card.content).padding(card.padding)
        ])
        .style(container::bordered_box)
        .clip(true)
        .into()
    }
}
