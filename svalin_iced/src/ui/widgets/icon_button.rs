use iced::{
    Length,
    widget::{Button, Text, Void, button, container, tooltip},
};

use crate::{
    bootstrap,
    ui::widgets::header::{HEADER_HEIGHT, HEADER_PADDING},
};

pub struct IconButton<'a, Message, Tip = Void> {
    icon: iced::widget::Text<'a>,
    tooltip: Tip,
    on_press: Option<Message>,
    size: iced::Pixels,
    selected: bool,
    danger: bool,
}

pub fn back<'a, Message>() -> IconButton<'a, Message> {
    IconButton::new(bootstrap::arrow_left())
}

impl<'a, Message> IconButton<'a, Message> {
    pub fn new(icon: iced::widget::Text<'a>) -> Self {
        Self {
            icon,
            on_press: None,
            tooltip: iced::widget::void(),
            size: (HEADER_HEIGHT - 2.0 * HEADER_PADDING).into(),
            selected: false,
            danger: false,
        }
    }
}

impl<'a, Message, Tip> IconButton<'a, Message, Tip> {
    pub fn on_press(mut self, on_press: Message) -> Self {
        self.on_press = Some(on_press);
        self
    }

    pub fn tooltip(self, tooltip: Text<'a>) -> IconButton<'a, Message, Text<'a>> {
        IconButton {
            icon: self.icon,
            tooltip,
            on_press: self.on_press,
            size: self.size,
            selected: self.selected,
            danger: self.danger,
        }
    }

    pub fn size(mut self, size: impl Into<iced::Pixels>) -> Self {
        self.size = size.into();
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }
}

impl<'a, Message: Clone + 'static, Tip> IconButton<'a, Message, Tip> {
    fn into_parts(self) -> (Button<'a, Message, Text<'a>>, Tip) {
        let value = self;
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
        let button = if value.danger {
            button.style(button::danger)
        } else if value.selected {
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
        (button, value.tooltip)
    }
}

impl<'a, Message: Clone + 'static> IconButton<'a, Message, Void> {
    pub fn into_widget(self) -> impl iced::Widget<Message> + 'a {
        self.into_parts().0
    }
}

impl<'a, Message: Clone + 'static> IconButton<'a, Message, Text<'a>> {
    pub fn into_widget(self) -> impl iced::Widget<Message> + 'a {
        let (button, tip) = self.into_parts();
        tooltip(
            button,
            container(tip)
                .padding(8)
                .style(|_| iced::widget::container::Style {
                    background: Some(iced::Color::BLACK.scale_alpha(0.70).into()),
                    text_color: Some(iced::Color::WHITE),
                    border: iced::border::rounded(4),
                    ..Default::default()
                }),
            tooltip::Position::Top,
        )
    }
}
