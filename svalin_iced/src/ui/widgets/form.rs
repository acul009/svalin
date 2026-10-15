use std::borrow::Cow;

use iced::{
    Element, Length, Widget,
    widget::{self, column, text},
};

pub struct Form<'a, Message> {
    title: Option<Cow<'a, str>>,

    control: Vec<Element<'a, Message>>,
    buttons: Vec<Element<'a, Message>>,
}

impl<'a, Message> Form<'a, Message> {
    pub(super) fn new() -> Self {
        Self {
            title: None,
            control: vec![],
            buttons: vec![],
        }
    }

    pub fn title(mut self, title: impl Into<Cow<'a, str>>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn control(mut self, control: impl Widget<Message> + 'a) -> Self {
        self.control.push(control.boxed());
        self
    }

    pub fn control_maybe(mut self, control: Option<impl Widget<Message> + 'a>) -> Self {
        if let Some(control) = control {
            self.control.push(control.boxed());
        }
        self
    }

    pub fn button(mut self, button: impl Widget<Message> + 'a) -> Self {
        self.buttons.push(button.boxed());
        self
    }
}

impl<'a, Message: Clone + 'static> Form<'a, Message> {
    pub fn into_widget(self) -> impl iced::Widget<Message> + 'a {
        let form = self;
        // let mut content_col = Column::with_capacity(3 + form.control.len() * 2).width(Length::Fill);

        let content = form
            .title
            .map(|title| text(title).boxed())
            .into_iter()
            .chain(form.control);

        let content_row = widget::row([column(content).width(Length::Fill).spacing(16)])
            .spacing(8)
            .height(Length::Fill)
            .width(Length::Fill);
        let buttons = [widget::space::horizontal().boxed()]
            .into_iter()
            .chain(form.buttons);
        let button_row = widget::row(buttons).spacing(4);

        widget::container(column![content_row, button_row].spacing(32))
            .padding(16)
            .width(Length::Fill)
    }
}
