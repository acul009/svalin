use iced::{
    Background, Border, Color, Length,
    alignment::Vertical,
    widget::{Text, container, row},
};

use crate::{
    Element, bootstrap,
    ui::{ERROR_COLOR, INFO_COLOR, SUCCESS_COLOR, WARNING_COLOR},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Info,
    Success,
    Warning,
    Error,
}

impl Kind {
    fn color(self) -> Color {
        match self {
            Self::Info => INFO_COLOR,
            Self::Success => SUCCESS_COLOR,
            Self::Warning => WARNING_COLOR,
            Self::Error => ERROR_COLOR,
        }
    }

    fn icon(self) -> Text<'static> {
        match self {
            Self::Info => bootstrap::info_circle(),
            Self::Success => bootstrap::check_circle(),
            Self::Warning => bootstrap::exclamation_triangle(),
            Self::Error => bootstrap::x_circle(),
        }
    }
}

pub struct Toast<'a, Message> {
    kind: Kind,
    content: Element<'a, Message>,
}

impl<'a, Message> Toast<'a, Message> {
    pub fn new(kind: Kind, content: impl Into<Element<'a, Message>>) -> Self {
        Self {
            kind,
            content: content.into(),
        }
    }
}

impl<'a, Message> From<Toast<'a, Message>> for Element<'a, Message>
where
    Message: 'a + 'static,
{
    fn from(toast: Toast<'a, Message>) -> Self {
        let kind = toast.kind;
        let accent = kind.color();
        let icon = container(kind.icon().size(30).color(accent))
            .width(60)
            .align_y(Vertical::Center);

        container(row![icon, toast.content].align_y(iced::Alignment::Center))
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fit)
            .style(move |_| container::Style {
                background: Some(Background::Color(accent.scale_alpha(0.05))),
                border: Border {
                    color: accent.scale_alpha(0.8),
                    width: 1.0,
                    radius: 7.0.into(),
                },
                ..Default::default()
            })
            .into()
    }
}
