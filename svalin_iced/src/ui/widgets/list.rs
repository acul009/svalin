use iced::{
    Length, Padding, Widget,
    alignment::Vertical,
    widget::{column, container},
};

use crate::{Element, Theme};

pub struct List<'a, Message> {
    entries: Vec<Element<'a, Message>>,
    entry_height: Length,
    entry_padding: Padding,
    width: Length,
}

impl<'a, Message> List<'a, Message> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            entry_height: 40.into(),
            width: Length::Fill,
            entry_padding: Padding::default(),
        }
    }

    pub(crate) fn with_children(
        children: impl IntoIterator<Item = impl Widget<Message> + 'a>,
    ) -> List<'a, Message> {
        Self {
            entries: children.into_iter().map(Widget::boxed).collect(),
            ..Self::new()
        }
    }

    pub fn push(mut self, entry: impl Widget<Message> + 'a) -> Self {
        self.entries.push(entry.boxed());
        self
    }

    pub fn entry_height(mut self, height: impl Into<Length>) -> Self {
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

impl<'a, Message: Clone + 'static> List<'a, Message> {
    pub fn into_widget(self) -> impl iced::Widget<Message> + 'a {
        let value = self;
        let mut col = column!();
        let mut odd = true;
        for entry in value.entries {
            col = col.push(
                container(entry)
                    .align_y(Vertical::Center)
                    .style(if odd { odd_style } else { even_style })
                    .height(value.entry_height)
                    .padding(value.entry_padding),
            );
            odd = !odd;
        }
        col.width(value.width)
    }
}

fn odd_style(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(theme.palette().background.weak.color.into()),
        ..Default::default()
    }
}

fn even_style(_theme: &Theme) -> container::Style {
    container::Style {
        ..Default::default()
    }
}
