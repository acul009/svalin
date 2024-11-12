use std::borrow::Cow;

use iced::{
    widget::{self, column, text, Column, Row},
    Element, Length,
};

pub struct Form<'a, Message> {
    title: Option<Cow<'a, str>>,
    controls: Vec<Element<'a, Message>>,
    primary_action: Option<Element<'a, Message>>,
    secondary_action: Option<Element<'a, Message>>,
    tertiary_action: Option<Element<'a, Message>>,
}

impl<'a, Message> Form<'a, Message> {
    pub fn new() -> Self {
        Self {
            title: None,
            controls: vec![],
            primary_action: None,
            secondary_action: None,
            tertiary_action: None,
        }
    }

    pub fn title(mut self, title: impl Into<Cow<'a, str>>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn control(mut self, control: impl Into<Element<'a, Message>>) -> Self {
        self.controls.push(control.into());
        self
    }

    pub fn primary_action(mut self, button: impl Into<Element<'a, Message>>) -> Self {
        self.primary_action = Some(button.into());
        self
    }

    pub fn secondary_action(mut self, button: impl Into<Element<'a, Message>>) -> Self {
        self.secondary_action = Some(button.into());
        self
    }

    pub fn tertiary_action(mut self, button: impl Into<Element<'a, Message>>) -> Self {
        self.tertiary_action = Some(button.into());
        self
    }
}

impl<'a, Message: Clone + 'static> From<Form<'a, Message>> for Element<'a, Message> {
    fn from(form: Form<'a, Message>) -> Self {
        let mut content_col = Column::with_capacity(3 + form.controls.len() * 2);

        let mut should_space = false;

        if let Some(title) = form.title {
            content_col = content_col.push(text(title));
            should_space = true;
        }
        for control in form.controls {
            if should_space {
                content_col = content_col.push(widget::vertical_space().height(16));
            }
            content_col = content_col.push(control);
            should_space = true;
        }

        let mut content_row = Row::with_capacity(2).spacing(8).height(Length::Fill);
        content_row = content_row.push(content_col);

        let mut button_row = Row::with_capacity(4).spacing(4);
        if let Some(button) = form.tertiary_action {
            button_row = button_row.push(button);
        }
        button_row = button_row.push(widget::horizontal_space().width(Length::Fill));
        if let Some(button) = form.secondary_action {
            button_row = button_row.push(button);
        }
        if let Some(button) = form.primary_action {
            button_row = button_row.push(button);
        }

        Element::from(widget::container(column![content_row, button_row].spacing(32)).padding(16))
    }
}
