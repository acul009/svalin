use iced::{
    Color, Length, Padding, Widget,
    widget::{center, column, container, float, opaque, row},
};

use crate::{
    Element, bootstrap,
    ui::widgets::{card, icon_button},
};

pub struct Dialog<'a, Message, Body, Title = iced::widget::Void> {
    body: Body,
    buttons: Vec<Element<'a, Message>>,
    title: Title,
    on_close: Option<Message>,
    width: Length,
    height: Length,
    padding: Padding,
    display: Display,
}

pub enum Display {
    Normal,
    Overlay,
    Float,
}

impl<'a, Message, Body> Dialog<'a, Message, Body> {
    pub(super) fn new(body: Body) -> Self {
        Self {
            body,
            buttons: vec![],
            title: iced::widget::void(),
            on_close: None,
            width: 500.into(),
            height: 300.into(),
            padding: 16.into(),
            display: Display::Normal,
        }
    }
}

impl<'a, Message, Body, Title> Dialog<'a, Message, Body, Title> {
    pub fn title<NewTitle>(self, title: NewTitle) -> Dialog<'a, Message, Body, NewTitle> {
        Dialog {
            body: self.body,
            buttons: self.buttons,
            title,
            on_close: self.on_close,
            width: self.width,
            height: self.height,
            padding: self.padding,
            display: self.display,
        }
    }

    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    pub fn button(mut self, button: impl Widget<Message> + 'a) -> Self {
        self.buttons.push(button.boxed());
        self
    }

    pub fn on_close(mut self, on_close: Message) -> Self {
        self.on_close = Some(on_close);
        self
    }

    pub fn on_close_maybe(mut self, on_close: Option<Message>) -> Self {
        self.on_close = on_close;
        self
    }

    pub fn width(mut self, max_width: impl Into<Length>) -> Self {
        self.width = max_width.into();
        self
    }

    pub fn height(mut self, max_height: impl Into<Length>) -> Self {
        self.height = max_height.into();
        self
    }

    pub fn fill(mut self) -> Self {
        self.width = Length::Fill;
        self.height = Length::Fill;
        self
    }

    pub fn float(mut self) -> Self {
        self.display = Display::Float;
        self
    }

    pub(crate) fn overlay(mut self) -> Self {
        self.display = Display::Overlay;
        self
    }

    pub fn display(mut self, display: Display) -> Self {
        self.display = display;
        self
    }
}

impl<'a, Message: Clone + 'static, Body, Title> Dialog<'a, Message, Body, Title>
where
    Body: Widget<Message> + 'a,
    Title: Widget<Message> + 'a,
{
    pub fn into_widget(self) -> impl iced::Widget<Message> + 'a {
        let value = self;
        let dialog = card(
            column![
                value.body,
                if !value.buttons.is_empty() {
                    Some(row(value.buttons).spacing(20))
                } else {
                    None
                }
            ]
            .spacing(20),
        )
        .title(value.title)
        .action(value.on_close.map(|message| {
            icon_button(bootstrap::x_lg())
                .on_press(message)
                .into_widget()
        }))
        .width(value.width)
        .padding(value.padding)
        .height(value.height)
        .into_widget();

        match value.display {
            Display::Normal => dialog.into(),
            Display::Float => float(overlay(dialog)).into(),
            Display::Overlay => overlay(dialog),
        }
    }
}

fn overlay<'a, Message: 'static>(dialog: impl Widget<Message> + 'a) -> impl Widget<Message> + 'a {
    opaque(center(dialog).style(|theme| {
        container::Style {
            background: Some(
                theme
                    .palette()
                    .background
                    .base
                    .color
                    .scale_alpha(0.8)
                    .mix(Color::BLACK, 0.5)
                    .into(),
            ),
            ..Default::default()
        }
    }))
}
