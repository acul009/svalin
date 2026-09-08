use iced::{
    Length,
    widget::{button, tooltip},
};

use crate::{
    Element, bootstrap,
    ui::widgets::scaffold::{HEADER_HEIGHT, HEADER_PADDING},
};

pub struct IconButton<'a, Message> {
    icon: iced::widget::Text<'a>,
    tooltip: Option<Element<'a, Message>>,
    on_press: Option<Message>,
    size: iced::Pixels,
}

pub fn back<'a, Message>() -> IconButton<'a, Message> {
    IconButton::new(bootstrap::arrow_left())
}

impl<'a, Message> IconButton<'a, Message> {
    pub fn new(icon: iced::widget::Text<'a>) -> Self {
        Self {
            icon,
            on_press: None,
            tooltip: None,
            size: (HEADER_HEIGHT - 2.0 * HEADER_PADDING).into(),
        }
    }

    pub fn on_press(mut self, on_press: Message) -> Self {
        self.on_press = Some(on_press);
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<Element<'a, Message>>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn size(mut self, size: impl Into<iced::Pixels>) -> Self {
        self.size = size.into();
        self
    }
}

impl<'a, Message: Clone + 'static> From<IconButton<'a, Message>> for Element<'a, Message> {
    fn from(value: IconButton<'a, Message>) -> Self {
        let button = button(
            value
                .icon
                .size(value.size.0 * 0.7)
                .width(Length::Fill)
                .height(Length::Fill)
                .center(),
        )
        .width(value.size)
        .height(value.size)
        .on_press_maybe(value.on_press);
        if let Some(tip) = value.tooltip {
            tooltip(button, tip, tooltip::Position::Top).into()
        } else {
            button.into()
        }
    }
}
