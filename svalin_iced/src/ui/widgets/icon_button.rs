use iced::{
    Length,
    widget::{button, tooltip},
};

use crate::{
    Element, bootstrap,
    ui::widgets::header::{HEADER_HEIGHT, HEADER_PADDING},
};

pub struct IconButton<'a, Message> {
    icon: iced::widget::Text<'a>,
    tooltip: Option<Element<'a, Message>>,
    on_press: Option<Message>,
    size: iced::Pixels,
    selected: bool,
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
            selected: false,
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

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
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
        let button = if value.selected {
            button.style(|theme: &iced::Theme, status| {
                let palette = theme.palette();
                let mut style = button::primary(theme, status);
                let darken = match status {
                    button::Status::Active => 0.50,
                    button::Status::Hovered => 0.40,
                    button::Status::Pressed => 0.60,
                    button::Status::Disabled => 0.0,
                };
                if !matches!(status, button::Status::Disabled) {
                    style.background = Some(
                        palette
                            .primary
                            .base
                            .color
                            .mix(iced::Color::BLACK, darken)
                            .into(),
                    );
                }
                style
            })
        } else {
            button
        };
        if let Some(tip) = value.tooltip {
            tooltip(button, tip, tooltip::Position::Top)
                .padding(8)
                .style(|_| iced::widget::container::Style {
                    background: Some(iced::Color::BLACK.scale_alpha(0.70).into()),
                    text_color: Some(iced::Color::WHITE),
                    border: iced::border::rounded(4),
                    ..Default::default()
                })
                .into()
        } else {
            button.into()
        }
    }
}
