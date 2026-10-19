use iced::{
    Length, Widget,
    widget::{Text, column, space},
};

use super::icon_button;

pub const RAIL_WIDTH: f32 = 60.0;
pub const RAIL_PADDING: f32 = 10.0;

pub struct Item<'a, Message> {
    icon: Text<'a>,
    tooltip: Option<Text<'a>>,
    selected: bool,
    on_press: Option<Message>,
}

impl<'a, Message> Item<'a, Message> {
    pub fn new(icon: Text<'a>) -> Self {
        Self {
            icon,
            tooltip: None,
            selected: false,
            on_press: None,
        }
    }

    pub fn tooltip(mut self, tooltip: Text<'a>) -> Self {
        self.tooltip = Some(tooltip);
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn on_press(mut self, message: Message) -> Self {
        self.on_press = Some(message);
        self
    }
}

impl<'a, Message: Clone + 'static> Item<'a, Message> {
    pub fn into_widget(self) -> impl Widget<Message> + 'a {
        let mut button = icon_button(self.icon)
            .size(RAIL_WIDTH - 2.0 * RAIL_PADDING)
            .selected(self.selected);
        if let Some(message) = self.on_press {
            button = button.on_press(message);
        }

        match self.tooltip {
            Some(tooltip) => button.tooltip(tooltip).into_widget().boxed(),
            None => button.into_widget().boxed(),
        }
    }
}

pub struct Rail<A, B> {
    items: A,
    bottom_items: B,
}

impl<'a, Message> Rail<std::iter::Empty<Item<'a, Message>>, std::iter::Empty<Item<'a, Message>>> {
    pub fn new() -> Self {
        Self {
            items: std::iter::empty(),
            bottom_items: std::iter::empty(),
        }
    }
}

impl<'a, Message, A, B> Rail<A, B>
where
    A: Iterator<Item = Item<'a, Message>>,
    B: Iterator<Item = Item<'a, Message>>,
{
    pub fn items<I: IntoIterator>(self, items: I) -> Rail<I::IntoIter, B> {
        Rail {
            items: items.into_iter(),
            bottom_items: self.bottom_items,
        }
    }

    pub fn bottom_items<I: IntoIterator>(self, items: I) -> Rail<A, I::IntoIter> {
        Rail {
            items: self.items,
            bottom_items: items.into_iter(),
        }
    }

    pub fn into_widget(self) -> impl Widget<Message> + 'a
    where
        Message: Clone + 'static,
    {
        column(
            self.items
                .map(|item| item.into_widget().boxed())
                .chain(std::iter::once(space::vertical().boxed()))
                .chain(self.bottom_items.map(|item| item.into_widget().boxed())),
        )
        .padding(RAIL_PADDING)
        .spacing(RAIL_PADDING)
        .width(RAIL_WIDTH)
        .height(Length::Fill)
    }
}
