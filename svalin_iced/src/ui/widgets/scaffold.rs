use iced::{
    Color, Length, Shadow, border,
    widget::{column, container, horizontal_rule, row, stack, vertical_rule},
};

use crate::Element;

pub struct Scaffold<'a, Message> {
    body: Element<'a, Message>,
    header: Option<Element<'a, Message>>,
    footer: Option<Element<'a, Message>>,
    dialog: Option<Element<'a, Message>>,
    context: Option<Element<'a, Message>>,
}

impl<'a, Message> Scaffold<'a, Message> {
    pub fn new(body: impl Into<Element<'a, Message>>) -> Self {
        Self {
            body: body.into(),
            header: None,
            footer: None,
            dialog: None,
            context: None,
        }
    }

    pub fn header(mut self, header: impl Into<Element<'a, Message>>) -> Self {
        self.header = Some(header.into());
        self
    }

    pub fn header_maybe(mut self, header: Option<impl Into<Element<'a, Message>>>) -> Self {
        if let Some(header) = header {
            self.header = Some(header.into());
        }
        self
    }

    pub fn footer(mut self, footer: impl Into<Element<'a, Message>>) -> Self {
        self.footer = Some(footer.into());
        self
    }

    pub fn footer_maybe(mut self, footer: Option<impl Into<Element<'a, Message>>>) -> Self {
        if let Some(footer) = footer {
            self.footer = Some(footer.into());
        }
        self
    }

    pub fn dialog(mut self, dialog: impl Into<Element<'a, Message>>) -> Self {
        self.dialog = Some(dialog.into());
        self
    }

    pub fn dialog_maybe(mut self, dialog: Option<impl Into<Element<'a, Message>>>) -> Self {
        if let Some(dialog) = dialog {
            self.dialog = Some(dialog.into());
        }
        self
    }

    pub fn context(mut self, context: impl Into<Element<'a, Message>>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn context_maybe(mut self, context: Option<impl Into<Element<'a, Message>>>) -> Self {
        if let Some(context) = context {
            self.context = Some(context.into());
        }
        self
    }
}

impl<'a, Message: Clone + 'static> From<Scaffold<'a, Message>> for Element<'a, Message> {
    fn from(scaffold: Scaffold<'a, Message>) -> Self {
        let has_header = scaffold.header.is_some();
        let has_context = scaffold.context.is_some();
        stack![
            column![
                scaffold.header.map(|header| {
                    container(header)
                        .style(|_| container::Style {
                            shadow: Shadow {
                                blur_radius: 50.0,
                                color: Color::BLACK.scale_alpha(0.5),
                                offset: iced::Vector { x: 0.0, y: 10.0 },
                            },
                            ..Default::default()
                        })
                        .height(50)
                }),
                has_header.then(|| horizontal_rule(2)),
                row![
                    scaffold.body,
                    has_context.then(|| vertical_rule(2)),
                    scaffold.context.map(|context| {
                        {
                            container(context)
                                .style(|_| container::Style {
                                    shadow: Shadow {
                                        blur_radius: 50.0,
                                        color: Color::BLACK.scale_alpha(0.5),
                                        offset: iced::Vector { x: -10.0, y: 0.0 },
                                    },
                                    ..Default::default()
                                })
                                .width(400)
                                .height(Length::Fill)
                        }
                    })
                ],
                scaffold.footer
            ],
            scaffold.dialog
        ]
        .into()
    }
}
