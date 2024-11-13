use std::borrow::Cow;

use iced::{
    widget::{button, text},
    Element,
};

use crate::fl;

use super::form;

pub struct ErrorDisplay<'a, Message> {
    title: Cow<'a, str>,
    error: &'a anyhow::Error,
    on_close: Option<Message>,
}

impl<'a, Message> ErrorDisplay<'a, Message> {
    pub(super) fn new(error: &'a anyhow::Error) -> Self {
        Self {
            error,
            on_close: None,
            title: fl!("error-generic").into(),
        }
    }

    pub fn title(mut self, title: impl Into<Cow<'a, str>>) -> Self {
        self.title = title.into();
        self
    }

    pub fn error(mut self, error: &'a anyhow::Error) -> Self {
        self.error = error;
        self
    }

    pub fn on_close(mut self, on_close: Message) -> Self {
        self.on_close = Some(on_close);
        self
    }
}

impl<'a, Message: Clone + 'static> From<ErrorDisplay<'a, Message>> for Element<'a, Message> {
    fn from(display: ErrorDisplay<'a, Message>) -> Self {
        let mut close_button = button(text(fl!("close")));
        if let Some(on_close) = display.on_close {
            close_button = close_button.on_press(on_close);
        }

        form()
            .title(display.title)
            .control(text(display.error.to_string()))
            .primary_action(close_button)
            .into()
    }
}
