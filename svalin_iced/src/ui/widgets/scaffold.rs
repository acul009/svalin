use iced::{
    border,
    widget::{self, button, column, container, row, text},
    Color, Length, Shadow,
};

use crate::Element;

pub struct Scaffold<'a, Message> {
    body: Element<'a, Message>,
    header: Option<Element<'a, Message>>,
    footer: Option<Element<'a, Message>>,
    on_back: Option<Message>,
}

impl<'a, Message> Scaffold<'a, Message> {
    pub fn new(body: impl Into<Element<'a, Message>>) -> Self {
        Self {
            body: body.into(),
            header: None,
            footer: None,
            on_back: None,
        }
    }

    pub fn header(mut self, header: impl Into<Element<'a, Message>>) -> Self {
        self.header = Some(header.into());
        self
    }

    pub fn footer(mut self, footer: impl Into<Element<'a, Message>>) -> Self {
        self.footer = Some(footer.into());
        self
    }

    pub fn on_back(mut self, on_back: Message) -> Self {
        self.on_back = Some(on_back);
        self
    }
}

impl<'a, Message: Clone + 'static> From<Scaffold<'a, Message>> for Element<'a, Message> {
    fn from(scaffold: Scaffold<'a, Message>) -> Self {
        container(
            column!()
                .push_maybe(if scaffold.on_back.is_some() || scaffold.header.is_some() {
                    let mut row = row!().height(50).width(Length::Fill);

                    if let Some(on_back) = scaffold.on_back {
                        row = row
                            .push(
                                button(text("<").width(Length::Fill).height(Length::Fill).center())
                                    .on_press(on_back)
                                    .height(Length::Fill)
                                    .width(50),
                            )
                            .push(widget::vertical_rule(2));
                    }

                    if let Some(header) = scaffold.header {
                        row = row.push(header);
                    };

                    Some(container(row).style(|_| container::Style {
                        text_color: None,
                        background: None,
                        border: border::width(0),
                        shadow: Shadow {
                            blur_radius: 50.0,
                            color: Color::BLACK.scale_alpha(0.5),
                            offset: iced::Vector { x: 0.0, y: 10.0 },
                        },
                    }))
                } else {
                    None
                })
                .push(scaffold.body)
                .push_maybe(scaffold.footer),
        )
        .into()
    }
}
