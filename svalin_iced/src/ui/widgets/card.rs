use iced::{
    Length, Padding,
    alignment::Vertical,
    widget::{center, column, container, row, space, stack},
};

use crate::{
    Element,
    ui::widgets::scaffold::{HEADER_HEIGHT, HEADER_PADDING},
};

pub struct Card<'a, Message> {
    title: Option<Element<'a, Message>>,
    content: Element<'a, Message>,
    padding: Padding,
    width: Length,
    height: Length,
    actions: Vec<Element<'a, Message>>,
}

impl<'a, Message> Card<'a, Message> {
    pub fn new(content: impl Into<Element<'a, Message>>) -> Self {
        Self {
            title: None,
            content: content.into(),
            padding: 16.into(),
            width: Length::Fill,
            height: Length::Fit,
            actions: Vec::new(),
        }
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    pub fn title(mut self, title: impl Into<Element<'a, Message>>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn action(mut self, action: impl Into<Element<'a, Message>>) -> Self {
        self.actions.push(action.into());
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
            stack![
                center(card.title).style(|theme| container::Style {
                    background: Some(theme.palette().background.strong.color.into()),
                    ..Default::default()
                }),
                row![space::horizontal()]
                    .extend(card.actions)
                    .height(Length::Fill)
                    .align_y(Vertical::Center)
                    .spacing(HEADER_PADDING)
                    .padding([0.0, HEADER_PADDING])
            ]
            .height(HEADER_HEIGHT),
            container(card.content).padding(card.padding)
        ])
        .style(container::bordered_box)
        .clip(true)
        .width(card.width)
        .height(card.height)
        .into()
    }
}
