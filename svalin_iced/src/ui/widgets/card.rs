use iced::{
    Length, Padding, Widget,
    widget::{column, container},
};

use crate::{Element, ui::widgets::header};

pub struct Card<'a, Message, Content, Title = iced::widget::Void> {
    title: Option<Title>,
    content: Content,
    padding: Padding,
    width: Length,
    height: Length,
    actions: Vec<Element<'a, Message>>,
}

impl<'a, Message, Content> Card<'a, Message, Content> {
    pub fn new(content: Content) -> Self {
        Self {
            title: None,
            content,
            padding: 16.into(),
            width: Length::Fill,
            height: Length::Fit,
            actions: Vec::new(),
        }
    }
}

impl<'a, Message, Content, Title> Card<'a, Message, Content, Title> {
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    pub fn title<NewTitle>(self, title: NewTitle) -> Card<'a, Message, Content, NewTitle> {
        Card {
            title: Some(title),
            content: self.content,
            padding: self.padding,
            width: self.width,
            height: self.height,
            actions: self.actions,
        }
    }

    pub fn action(mut self, action: impl Widget<Message> + 'a) -> Self {
        self.actions.push(action.boxed());
        self
    }

    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }
}

impl<'a, Message, Content, Title> Card<'a, Message, Content, Title>
where
    Message: 'static + Clone,
    Content: Widget<Message> + 'a,
    Title: Widget<Message> + 'a,
{
    pub fn into_widget(self) -> impl iced::Widget<Message> + 'a {
        let card = self;
        container(column![
            container(header(card.title).with_actions(card.actions).into_widget()).style(
                |theme: &iced::Theme| {
                    container::Style {
                        background: Some(theme.palette().background.strong.color.into()),
                        ..Default::default()
                    }
                }
            ),
            container(card.content).padding(card.padding)
        ])
        .style(container::bordered_box)
        .clip(true)
        .width(card.width)
        .height(card.height)
    }
}
