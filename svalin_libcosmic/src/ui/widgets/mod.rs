use std::borrow::Cow;

pub mod error_display;
pub mod form;
pub mod loading;

pub fn form<'a, Message>() -> form::Form<'a, Message> {
    form::Form::new()
}

pub fn error_display<'a, Message>(
    error: &'a anyhow::Error,
) -> error_display::ErrorDisplay<'a, Message> {
    error_display::ErrorDisplay::new(error)
}

pub fn loading<'a>(message: impl Into<Cow<'a, str>>) -> loading::Loading<'a> {
    loading::Loading::new(message)
}
