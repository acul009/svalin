use crate::ui::widgets::{error_display, error_display::ErrorDisplay};

#[derive(Debug, Clone)]
pub struct ErrorDisplayInfo<E> {
    error: E,
    context: String,
}

impl<E> ErrorDisplayInfo<E> {
    pub fn new(error: E, context: impl Into<String>) -> Self {
        Self {
            error,
            context: context.into(),
        }
    }

    pub fn error(&self) -> &E {
        &self.error
    }

    pub fn context(&self) -> &str {
        &self.context
    }
}

impl<E> ErrorDisplayInfo<E>
where
    E: std::fmt::Display,
{
    pub fn view<Message>(&self) -> ErrorDisplay<E, Message> {
        error_display(self.error()).title(self.context()).into()
    }
}
