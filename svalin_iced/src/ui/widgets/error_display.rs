use std::borrow::Cow;

use iced::widget::text;

use crate::ui::widgets::dialog;

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
            title: t!("generic.error").into(),
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

impl<'a, Error, Message: Clone + 'static> ErrorDisplay<'a, Error, Message>
where
    Error: std::fmt::Display,
{
    /// Finishes the error display without an overlay wrapper.
    pub fn into_normal(self) -> impl iced::Widget<Message> + 'a {
        self.into_dialog().into_normal()
    }

    /// Finishes the error display as a centered overlay with a dimmed background.
    pub fn into_overlay(self) -> impl iced::Widget<Message> + 'a {
        self.into_dialog().into_overlay()
    }

    /// Finishes the error display as a floating overlay.
    pub fn into_float(self) -> impl iced::Widget<Message> + 'a {
        self.into_dialog().into_float()
    }

    fn into_dialog(
        self,
    ) -> dialog::Dialog<'a, Message, iced::widget::Text<'a>, iced::widget::Text<'a>> {
        let display = self;
        dialog(text!("{:#}", display.error))
            .title(text(display.title))
            .on_close_maybe(display.on_close)
    }
}
