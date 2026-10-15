use iced::{
    Length, Widget,
    alignment::Vertical,
    widget::{self, center, container, space, stack},
};

use crate::{Element, bootstrap, ui::widgets::icon_button};

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
    pub fn new(content: impl Widget<Message> + 'a) -> Self {
        Self {
            style: HeaderStyle::Default,
            content: content.boxed(),
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

    pub fn action(mut self, action: impl Widget<Message> + 'a) -> Self {
        self.actions.push(action.boxed());
        self
    }

    pub(crate) fn with_actions(mut self, actions: Vec<Element<'a, Message>>) -> Self {
        self.actions.extend(actions);
        self
    }

    pub fn map<NewMessage>(
        self,
        f: impl 'a + Copy + Fn(Message) -> NewMessage,
    ) -> Header<'a, NewMessage>
    where
        NewMessage: Clone + 'static,
    {
        let content = self.content.map(f).boxed();
        let on_back = self.on_back.map(f);
        let actions = self.actions.into_iter().map(|a| a.map(f).boxed()).collect();
        Header {
            style: self.style,
            content,
            on_back,
            actions,
        }
    }
}

pub const HEADER_HEIGHT: f32 = 60.0;
pub const HEADER_PADDING: f32 = 10.0;

impl<'a, Message: Clone + 'static> Header<'a, Message> {
    pub fn into_widget(self) -> impl iced::Widget<Message> + 'a {
        let header = self;
        let height = HEADER_HEIGHT;
        let padding = HEADER_PADDING;
        let icon_size = height - 2.0 * padding;
        let row = header
            .on_back
            .map(|on_back| {
                let icon = match header.style {
                    HeaderStyle::Default => bootstrap::arrow_left(),
                    HeaderStyle::Drawer => bootstrap::x(),
                };
                icon_button(icon)
                    .size(icon_size)
                    .on_press(on_back)
                    .into_widget()
                    .boxed()
            })
            .into_iter()
            .chain([space::horizontal().boxed()])
            .chain(header.actions);

        let row = widget::row(row)
            .align_y(Vertical::Center)
            .spacing(HEADER_PADDING)
            .width(Length::Fill)
            .height(Length::Fill);

        container(stack![center(header.content), row])
            .padding(padding)
            .height(height)
            .width(Length::Fill)
    }
}
