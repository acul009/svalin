use std::borrow::Cow;

use iced::{
    widget::{button, text},
    Element,
};

use super::form;

pub struct ErrorDisplay<'a, Error, Message> {
    title: Cow<'a, str>,
    error: &'a Error,
    on_close: Option<Message>,
}

impl<'a, Error, Message> ErrorDisplay<'a, Error, Message> {
    pub(super) fn new(error: &'a Error) -> Self {
        Self {
            error,
            on_close: None,
            title: t!("error-generic").into(),
        }
    }

    pub fn title(mut self, title: impl Into<Cow<'a, str>>) -> Self {
        self.title = title.into();
        self
    }

    pub fn error(mut self, error: &'a Error) -> Self {
        self.error = error;
        self
    }

    pub fn on_close(mut self, on_close: Message) -> Self {
        self.on_close = Some(on_close);
        self
    }
}

impl<'a, Error, Message: Clone + 'static> From<ErrorDisplay<'a, Error, Message>>
    for Element<'a, Message>
where
    Error: std::fmt::Display,
{
    fn from(display: ErrorDisplay<'a, Error, Message>) -> Self {
        let mut close_button = button(text(t!("close")));
        if let Some(on_close) = display.on_close {
            close_button = close_button.on_press(on_close);
        }

        form()
            .title(display.title)
            .control(text!("{:#}", display.error))
            .primary_action(close_button)
            .into()
    }
}
