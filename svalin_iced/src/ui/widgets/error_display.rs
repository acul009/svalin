use std::borrow::Cow;

use iced::widget::text;

use crate::ui::widgets::dialog;

pub struct ErrorDisplay<'a, Error, Message> {
    title: Cow<'a, str>,
    error: &'a Error,
    on_close: Option<Message>,
    display: dialog::Display,
}

impl<'a, Error, Message> ErrorDisplay<'a, Error, Message> {
    pub(super) fn new(error: &'a Error) -> Self {
        Self {
            error,
            on_close: None,
            title: t!("generic.error").into(),
            display: dialog::Display::Normal,
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

    pub fn float(mut self) -> Self {
        self.display = dialog::Display::Float;
        self
    }

    pub fn overlay(mut self) -> Self {
        self.display = dialog::Display::Overlay;
        self
    }
}

impl<'a, Error, Message: Clone + 'static> ErrorDisplay<'a, Error, Message>
where
    Error: std::fmt::Display,
{
    pub fn into_widget(self) -> impl iced::Widget<Message> + 'a {
        let display = self;
        dialog(text!("{:#}", display.error))
            .title(text(display.title))
            .on_close_maybe(display.on_close)
            .display(display.display)
            .into_widget()
    }
}
