use iced::{
    Background, Border, Color, Length, Widget,
    widget::{Text, container, row},
};

use crate::{
    bootstrap,
    ui::{ERROR_COLOR, INFO_COLOR, SUCCESS_COLOR, WARNING_COLOR},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Neutral,
    Info,
    Success,
    Warning,
    Error,
}

impl Kind {
    fn color(self, theme: &iced::Theme) -> Color {
        match self {
            Self::Neutral => theme.palette().background.neutral.color,
            Self::Info => INFO_COLOR,
            Self::Success => SUCCESS_COLOR,
            Self::Warning => WARNING_COLOR,
            Self::Error => ERROR_COLOR,
        }
    }

    fn icon(self) -> Option<Text<'static>> {
        Some(match self {
            Self::Neutral => return None,
            Self::Info => bootstrap::info_circle(),
            Self::Success => bootstrap::check_circle(),
            Self::Warning => bootstrap::exclamation_triangle(),
            Self::Error => bootstrap::x_circle(),
        })
    }
}

pub struct Toast<Content> {
    kind: Kind,
    content: Content,
}

impl<Content> Toast<Content> {
    pub fn new(kind: Kind, content: Content) -> Self {
        Self { kind, content }
    }
}

impl<Content> Toast<Content> {
    pub fn into_widget<'a, Message: 'static>(self) -> impl iced::Widget<Message> + 'a
    where
        Content: iced::Widget<Message> + 'a,
    {
        let toast = self;
        let kind = toast.kind;
        let content = kind
            .icon()
            .map(|icon| {
                icon.size(30)
                    .style(move |theme| iced::widget::text::Style {
                        color: Some(kind.color(theme)),
                        ..Default::default()
                    })
                    .width(60)
                    .boxed()
            })
            .into_iter()
            .chain([toast.content.boxed()]);

        container(row(content).align_y(iced::Alignment::Center))
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fit)
            .style(move |theme| {
                let accent = kind.color(theme);
                container::Style {
                    background: Some(Background::Color(match kind {
                        Kind::Neutral => accent.scale_alpha(0.1),
                        _ => accent.scale_alpha(0.05),
                    })),
                    border: Border {
                        color: accent.scale_alpha(0.8),
                        width: 1.0,
                        radius: 7.0.into(),
                    },
                    ..Default::default()
                }
            })
    }
}
