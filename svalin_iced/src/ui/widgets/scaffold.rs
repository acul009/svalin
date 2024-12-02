use iced::{
    border,
    widget::{column, container, horizontal_rule, pane_grid, row, stack, vertical_rule},
    Color, Length, Shadow,
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
        let mut col = column!();

        if let Some(header) = scaffold.header {
            col = col
                .push(
                    container(header)
                        .style(|_| container::Style {
                            text_color: None,
                            background: None,
                            border: border::width(0),
                            shadow: Shadow {
                                blur_radius: 50.0,
                                color: Color::BLACK.scale_alpha(0.5),
                                offset: iced::Vector { x: 0.0, y: 10.0 },
                            },
                        })
                        .height(50),
                )
                .push(horizontal_rule(2));
        }

        let mut r = row!(scaffold.body);

        if let Some(context) = scaffold.context {
            r = r.push(vertical_rule(2)).push(
                container(context)
                    .style(|_| container::Style {
                        text_color: None,
                        background: None,
                        border: border::width(0),
                        shadow: Shadow {
                            blur_radius: 50.0,
                            color: Color::BLACK.scale_alpha(0.5),
                            offset: iced::Vector { x: -10.0, y: 0.0 },
                        },
                    })
                    .width(400)
                    .height(Length::Fill),
            );
        }

        col = col.push(r).push_maybe(scaffold.footer);

        stack!(col).push_maybe(scaffold.dialog).into()
    }
}
