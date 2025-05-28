use std::borrow::Cow;

use iced::{
    Element, Length,
    widget::{self, Column, Row, column, text},
};

pub struct Form<'a, Message> {
    title: Option<Cow<'a, str>>,

    control: Vec<Element<'a, Message>>,
    buttons: Vec<Element<'a, Message>>,
}

impl<'a, Message> Form<'a, Message> {
    pub(super) fn new() -> Self {
        Self {
            title: None,
            control: vec![],
            buttons: vec![],
        }
    }

    pub fn title(mut self, title: impl Into<Cow<'a, str>>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn control(mut self, control: impl Into<Element<'a, Message>>) -> Self {
        self.control.push(control.into());
        self
    }

    pub fn control_maybe(mut self, control: Option<impl Into<Element<'a, Message>>>) -> Self {
        if let Some(control) = control {
            self.control.push(control.into());
        }
        self
    }

    pub fn button(mut self, button: impl Into<Element<'a, Message>>) -> Self {
        self.buttons.push(button.into());
        self
    }
}

impl<'a, Message: Clone + 'static> From<Form<'a, Message>> for Element<'a, Message> {
    fn from(form: Form<'a, Message>) -> Self {
        let mut content_col = Column::with_capacity(3 + form.control.len() * 2).width(Length::Fill);

        let mut should_space = false;

        if let Some(title) = form.title {
            content_col = content_col.push(text(title));
            should_space = true;
        }

        for control in form.control {
            if should_space {
                content_col = content_col.push(widget::vertical_space().height(16));
            }
            content_col = content_col.push(control);
            should_space = true;
        }

        let mut content_row = Row::with_capacity(2)
            .spacing(8)
            .height(Length::Fill)
            .width(Length::Fill);
        content_row = content_row.push(content_col);

        let mut button_row = Row::with_capacity(1 + form.buttons.len())
            .spacing(4)
            .push(widget::horizontal_space().width(Length::Fill));

        for button in form.buttons {
            button_row = button_row.push(button);
        }

        Element::from(
            widget::container(column![content_row, button_row].spacing(32))
                .padding(16)
                .width(Length::Fill),
        )
    }
}
