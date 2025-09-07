use std::borrow::Cow;

use iced::{
    Background, Border, Color, Shadow,
    widget::{center, container, text},
};

use crate::Element;

use super::form::Form;

pub struct Dialog<'a, Message> {
    form: Form<'a, Message>,
    body: Option<Cow<'a, str>>,
    controls: Vec<Element<'a, Message>>,
    max_width: iced::Pixels,
    max_height: iced::Pixels,
}

impl<'a, Message> Dialog<'a, Message> {
    pub(super) fn new() -> Self {
        Self {
            form: Form::new(),
            body: None,
            controls: vec![],
            max_width: 500.into(),
            max_height: 300.into(),
        }
    }

    pub fn title(mut self, title: impl Into<Cow<'a, str>>) -> Self {
        self.form = self.form.title(title);
        self
    }

    pub fn control(mut self, control: impl Into<Element<'a, Message>>) -> Self {
        self.controls.push(control.into());
        self
    }

    pub fn body(mut self, body: impl Into<Cow<'a, str>>) -> Self {
        self.body = Some(body.into());
        self
    }

    pub fn button(mut self, button: impl Into<Element<'a, Message>>) -> Self {
        self.form = self.form.button(button);
        self
    }

    pub fn max_width(mut self, max_width: impl Into<iced::Pixels>) -> Self {
        self.max_width = max_width.into();
        self
    }

    pub fn max_height(mut self, max_height: impl Into<iced::Pixels>) -> Self {
        self.max_height = max_height.into();
        self
    }
}

impl<'a, Message: Clone + 'static> From<Dialog<'a, Message>> for Element<'a, Message> {
    fn from(value: Dialog<'a, Message>) -> Self {
        center(
            container(
                value.controls.into_iter().fold(
                    value.form.control_maybe(
                        value
                            .body
                            .map(|body| text(body).wrapping(text::Wrapping::WordOrGlyph)),
                    ),
                    |form, control| form.control(control),
                ),
            )
            .style(|theme| container::Style {
                background: Some(Background::Color(theme.palette().background)),
                ..Default::default()
            })
            .max_width(value.max_width)
            .max_height(value.max_height),
        )
        .style(|_| container::Style {
            background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.5))),
            ..Default::default()
        })
        .into()
    }
}
