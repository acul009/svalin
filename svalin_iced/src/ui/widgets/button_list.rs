use iced::{
    Color, Length, Padding, Pixels,
    alignment::Vertical,
    widget::{button, column, container, row, text},
};

use crate::{Element, Theme, bootstrap};

pub struct ButtonList<'a, Message> {
    entries: Vec<Entry<'a, Message>>,
    entry_height: Pixels,
    entry_padding: Padding,
    width: Length,
}

pub struct Entry<'a, Message> {
    content: Element<'a, Message>,
    on_press: Option<Message>,
    color: Option<Color>,
}

pub fn entry<'a, Message>(content: impl Into<Element<'a, Message>>) -> Entry<'a, Message> {
    Entry {
        content: content.into(),
        on_press: None,
        color: None,
    }
}

impl<'a, Message> Entry<'a, Message> {
    pub fn on_press(mut self, on_press: Message) -> Self {
        self.on_press = Some(on_press);
        self
    }

    pub fn on_press_maybe(mut self, on_press: Option<Message>) -> Self {
        self.on_press = on_press;
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Gives the entry a subtle error-colored background.
    pub fn error(self) -> Self {
        self.color(crate::ui::ERROR_COLOR.scale_alpha(0.15))
    }
}

impl<'a, Message> ButtonList<'a, Message> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            entry_height: 50.into(),
            entry_padding: Padding::from([8, 16]),
            width: Length::Fill,
        }
    }

    pub(crate) fn with_children(
        entries: impl IntoIterator<Item = Entry<'a, Message>>,
    ) -> ButtonList<'a, Message> {
        Self {
            entries: entries.into_iter().collect(),
            ..Self::new()
        }
    }

    pub fn push(mut self, entry: Entry<'a, Message>) -> Self {
        self.entries.push(entry);
        self
    }

    /// Appends an entry when present, leaving the list unchanged for `None`.
    pub fn push_maybe(self, entry: Option<Entry<'a, Message>>) -> Self {
        match entry {
            Some(entry) => self.push(entry),
            None => self,
        }
    }

    pub fn entry_height(mut self, height: impl Into<Pixels>) -> Self {
        self.entry_height = height.into();
        self
    }

    pub fn entry_padding(mut self, padding: impl Into<Padding>) -> Self {
        self.entry_padding = padding.into();
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }
}

impl<'a, Message: Clone + 'static> From<ButtonList<'a, Message>> for Element<'a, Message> {
    fn from(list: ButtonList<'a, Message>) -> Self {
        let mut rows = column![];
        for (index, entry) in list.entries.into_iter().enumerate() {
            let odd = index % 2 == 0;
            let content = row![
                container(entry.content).width(Length::Fill),
                bootstrap::chevron_right()
                    .size(list.entry_height.0 * 0.48)
                    .width(list.entry_height.0 * 0.56)
                    .center()
                    .style(|theme: &Theme| text::Style {
                        color: Some(theme.palette().primary.base.color),
                    }),
            ]
            .spacing(16)
            .align_y(Vertical::Center);

            rows = rows.push(
                container(
                    button(content)
                        .on_press_maybe(entry.on_press)
                        .width(Length::Fill)
                        .height(list.entry_height)
                        .padding(list.entry_padding)
                        .style(if odd { row_style_odd } else { row_style_even }),
                )
                .width(Length::Fill)
                .style(move |_theme| container::Style {
                    background: entry.color.map(Into::into),
                    ..Default::default()
                }),
            );
        }
        rows.width(list.width).into()
    }
}

fn row_style_odd(theme: &Theme, status: button::Status) -> button::Style {
    let mut style = row_style_even(theme, status);
    if matches!(status, button::Status::Active) {
        style.background = Some(style.text_color.scale_alpha(0.04).into());
    }
    style
}

fn row_style_even(theme: &Theme, status: button::Status) -> button::Style {
    let foreground = theme.palette().background.base.text;
    let alpha = match status {
        button::Status::Active | button::Status::Disabled => 0.0,
        button::Status::Hovered => 0.10,
        button::Status::Pressed => 0.16,
    };
    button::Style {
        background: Some(foreground.scale_alpha(alpha).into()),
        text_color: foreground,
        ..Default::default()
    }
}
