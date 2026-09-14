use iced::{
    Color, Length, Shadow,
    alignment::Vertical,
    widget::{center, column, container, row, rule, space, stack},
};

use crate::{
    Element, bootstrap,
    ui::widgets::{
        icon_button,
        scaffold::{HEADER_HEIGHT, HEADER_PADDING},
    },
};

#[derive(Clone, Copy, Default)]
pub enum HeaderStyle {
    #[default]
    Default,
    Drawer,
}

pub struct Header<'a, Message> {
    style: HeaderStyle,
    content: Element<'a, Message>,
    on_back: Option<Message>,
    actions: Vec<Element<'a, Message>>,
}

impl<'a, Message: 'static> Header<'a, Message> {
    pub fn new(content: impl Into<Element<'a, Message>>) -> Self {
        Self {
            style: HeaderStyle::Default,
            content: content.into(),
            on_back: None,
            actions: Vec::new(),
        }
    }

    pub fn drawer(mut self) -> Self {
        self.style = HeaderStyle::Drawer;
        self
    }

    pub fn on_back(mut self, message: Message) -> Self {
        self.on_back = Some(message);
        self
    }

    pub fn on_back_maybe(mut self, message: Option<Message>) -> Self {
        if let Some(message) = message {
            self.on_back = Some(message);
        }
        self
    }

    pub fn action(mut self, action: impl Into<Element<'a, Message>>) -> Self {
        self.actions.push(action.into());
        self
    }

    pub fn map<NewMessage>(
        self,
        f: impl 'a + Copy + Fn(Message) -> NewMessage,
    ) -> Header<'a, NewMessage>
    where
        NewMessage: Clone + 'static,
    {
        let content = self.content.map(f);
        let on_back = self.on_back.map(f);
        let actions = self.actions.into_iter().map(|a| a.map(f)).collect();
        Header {
            style: self.style,
            content,
            on_back,
            actions,
        }
    }
}

impl<'a, Message: Clone + 'static> From<Header<'a, Message>> for Element<'a, Message> {
    fn from(header: Header<'a, Message>) -> Self {
        let height = HEADER_HEIGHT;
        let padding = HEADER_PADDING;
        let icon_size = height - 2.0 * padding;
        let mut row = row![];
        if let Some(on_back) = header.on_back {
            let icon = match header.style {
                HeaderStyle::Default => bootstrap::arrow_left(),
                HeaderStyle::Drawer => bootstrap::x(),
            };
            row = row.push(icon_button(icon).size(icon_size).on_press(on_back));
        }
        row = row
            .push(space::horizontal())
            .extend(header.actions)
            .align_y(Vertical::Center)
            .spacing(HEADER_PADDING)
            .width(Length::Fill)
            .height(Length::Fill);

        let cont = container(stack![center(header.content), row])
            .padding(padding)
            .height(height)
            .width(Length::Fill);
        match header.style {
            HeaderStyle::Drawer => cont.into(),
            HeaderStyle::Default => column![
                cont.style(|_| container::Style {
                    shadow: Shadow {
                        blur_radius: 50.0,
                        color: Color::BLACK.scale_alpha(0.5),
                        offset: iced::Vector { x: 0.0, y: 10.0 },
                    },
                    ..Default::default()
                }),
                rule::horizontal(2),
            ]
            .into(),
        }
    }
}
