use iced::{
    Length, Padding, Widget,
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
        title: impl Widget<Message> + 'a,
        value: impl Widget<Message> + 'a,
    ) -> Self {
        self.entries.push(Entry {
            icon: iced::widget::void().boxed(),
            title: title.boxed(),
            value: value.boxed(),
        });
        self
    }

    pub fn with_icon(
        mut self,
        icon: impl Widget<Message> + 'a,
        title: impl Widget<Message> + 'a,
        value: impl Widget<Message> + 'a,
    ) -> Self {
        self.entries.push(Entry {
            icon: icon.boxed(),
            title: title.boxed(),
            value: value.boxed(),
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

impl<'a, Message: Clone + 'static> FactList<'a, Message> {
    pub fn into_widget(self) -> impl iced::Widget<Message> + 'a {
        let value = self;
        let entries = value.entries.into_iter().map(|entry| {
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
            .height(Length::Fill)
        });

        List::with_children(entries)
            .entry_height(value.entry_height)
            .entry_padding(value.entry_padding)
            .width(value.width)
            .into_widget()
    }
}
