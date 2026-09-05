use iced::{
    Length, Padding,
    alignment::Vertical,
    widget::{container, row, space},
};

use crate::{Element, ui::widgets::list::List};

pub struct FactList<'a, Message> {
    entries: Vec<Entry<'a, Message>>,
    entry_height: Length,
    entry_padding: Padding,
    width: Length,
}

struct Entry<'a, Message> {
    icon: Element<'a, Message>,
    title: Element<'a, Message>,
    value: Element<'a, Message>,
}

impl<'a, Message> FactList<'a, Message> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            entry_height: 40.into(),
            width: Length::Fill,
            entry_padding: Padding::default(),
        }
    }

    pub fn entry(
        mut self,
        title: impl Into<Element<'a, Message>>,
        value: impl Into<Element<'a, Message>>,
    ) -> Self {
        self.entries.push(Entry {
            icon: iced::widget::void().into(),
            title: title.into(),
            value: value.into(),
        });
        self
    }

    pub fn with_icon(
        mut self,
        icon: impl Into<Element<'a, Message>>,
        title: impl Into<Element<'a, Message>>,
        value: impl Into<Element<'a, Message>>,
    ) -> Self {
        self.entries.push(Entry {
            icon: icon.into(),
            title: title.into(),
            value: value.into(),
        });
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

impl<'a, Message: Clone + 'static> From<FactList<'a, Message>> for Element<'a, Message> {
    fn from(value: FactList<'a, Message>) -> Self {
        let mut list = List::new();
        for entry in value.entries {
            list = list.push(
                row![
                    container(entry.icon)
                        .center(value.entry_height)
                        .height(Length::Fill),
                    entry.title,
                    space::horizontal(),
                    entry.value,
                    space().width(value.entry_height)
                ]
                .align_y(Vertical::Center)
                .height(Length::Fill),
            );
        }

        list.entry_height(value.entry_height)
            .entry_padding(value.entry_padding)
            .width(value.width)
            .into()
    }
}
