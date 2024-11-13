use std::borrow::Cow;

use iced::{
    widget::{center, container, text},
    Background, Border, Color, Length, Shadow,
};

use crate::Element;

use super::form::Form;

pub struct Dialog<'a, Message> {
    form: Form<'a, Message>,
    body: Option<Cow<'a, str>>,
    controls: Vec<Element<'a, Message>>,
}

impl<'a, Message> Dialog<'a, Message> {
    pub(super) fn new() -> Self {
        Self {
            form: Form::new(),
            body: None,
            controls: vec![],
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

    pub fn primary_action(mut self, button: impl Into<Element<'a, Message>>) -> Self {
        self.form = self.form.primary_action(button);
        self
    }

    pub fn secondary_action(mut self, button: impl Into<Element<'a, Message>>) -> Self {
        self.form = self.form.secondary_action(button);
        self
    }

    pub fn tertiary_action(mut self, button: impl Into<Element<'a, Message>>) -> Self {
        self.form = self.form.tertiary_action(button);
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
                            .map(|body| text(body).width(10).wrapping(text::Wrapping::WordOrGlyph)),
                    ),
                    |form, control| form.control(control),
                ),
            )
            .style(|theme| container::Style {
                text_color: None,
                background: Some(Background::Color(theme.palette().background)),
                border: Border::default(),
                shadow: Shadow::default(),
            })
            .max_width(400)
            .max_height(300),
        )
        .style(|_| container::Style {
            text_color: None,
            background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.5))),
            border: Border::default(),
            shadow: Shadow::default(),
        })
        .into()
    }
}
