use iced::{
    Length,
    alignment::Vertical,
    widget::{center, row},
};

use crate::{
    Element,
    ui::widgets::{
        icon_button,
        scaffold::{HEADER_HEIGHT, HEADER_PADDING},
    },
};

pub struct Header<'a, Message> {
    content: Element<'a, Message>,
    on_back: Option<Message>,
    actions: Vec<Element<'a, Message>>,
}

impl<'a, Message: 'static> Header<'a, Message> {
    pub fn new(content: impl Into<Element<'a, Message>>) -> Self {
        Self {
            content: content.into(),
            on_back: None,
            actions: Vec::new(),
        }
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
            content,
            on_back,
            actions,
        }
    }
}

impl<'a, Message: Clone + 'static> From<Header<'a, Message>> for Element<'a, Message> {
    fn from(header: Header<'a, Message>) -> Self {
        let mut row = match header.on_back {
            None => row!(),
            Some(on_back) => row![icon_button::back().on_press(on_back)],
        };

        row = row
            .push(center(header.content))
            .extend(header.actions)
            .align_y(Vertical::Center)
            .spacing(HEADER_PADDING)
            .width(Length::Fill)
            .height(HEADER_HEIGHT);

        row.into()
    }
}
