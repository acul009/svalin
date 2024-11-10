use std::borrow::Cow;

use cosmic::{cosmic_theme, iced::Length, style, theme, widget, Element, Theme};

pub struct Form<'a, Message> {
    title: Option<Cow<'a, str>>,
    controls: Vec<Element<'a, Message>>,
    primary_action: Option<Element<'a, Message>>,
    secondary_action: Option<Element<'a, Message>>,
    tertiary_action: Option<Element<'a, Message>>,
}

impl<'a, Message> Form<'a, Message> {
    pub fn new() -> Self {
        Self {
            title: None,
            controls: vec![],
            primary_action: None,
            secondary_action: None,
            tertiary_action: None,
        }
    }

    pub fn title(mut self, title: impl Into<Cow<'a, str>>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn control(mut self, control: impl Into<Element<'a, Message>>) -> Self {
        self.controls.push(control.into());
        self
    }

    pub fn primary_action(mut self, button: impl Into<Element<'a, Message>>) -> Self {
        self.primary_action = Some(button.into());
        self
    }

    pub fn secondary_action(mut self, button: impl Into<Element<'a, Message>>) -> Self {
        self.secondary_action = Some(button.into());
        self
    }

    pub fn tertiary_action(mut self, button: impl Into<Element<'a, Message>>) -> Self {
        self.tertiary_action = Some(button.into());
        self
    }
}

impl<'a, Message: Clone + 'static> From<Form<'a, Message>> for Element<'a, Message> {
    fn from(form: Form<'a, Message>) -> Self {
        let cosmic::cosmic_theme::Spacing {
            space_l,
            space_m,
            space_s,
            space_xxs,
            ..
        } = cosmic::theme::active().cosmic().spacing;

        let mut content_col = widget::column::with_capacity(3 + form.controls.len() * 2);

        let mut should_space = false;

        if let Some(title) = form.title {
            content_col = content_col.push(widget::text::title3(title));
            should_space = true;
        }
        for control in form.controls {
            if should_space {
                content_col = content_col
                    .push(widget::vertical_space().height(Length::Fixed(space_s.into())));
            }
            content_col = content_col.push(control);
            should_space = true;
        }

        let mut content_row = widget::row::with_capacity(2)
            .spacing(space_s)
            .height(Length::Fill);
        content_row = content_row.push(content_col);

        let mut button_row = widget::row::with_capacity(4).spacing(space_xxs);
        if let Some(button) = form.tertiary_action {
            button_row = button_row.push(button);
        }
        button_row = button_row.push(widget::horizontal_space().width(Length::Fill));
        if let Some(button) = form.secondary_action {
            button_row = button_row.push(button);
        }
        if let Some(button) = form.primary_action {
            button_row = button_row.push(button);
        }

        Element::from(
            widget::container(
                widget::column::with_children(vec![content_row.into(), button_row.into()])
                    .spacing(space_l),
            )
            .padding(space_m),
        )
    }
}
