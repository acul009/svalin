use std::{borrow::Cow, ops::RangeInclusive};

use iced::{
    Length, Padding,
    alignment::{self, Vertical},
    padding,
    widget::{column, container, progress_bar, stack, text},
};

use super::progress_circle;

pub struct PercentDisplay<'a, Subinfo = iced::widget::Void> {
    range: RangeInclusive<f32>,
    value: f32,
    size: f32,
    label: Cow<'a, str>,
    display_type: DisplayType,
    padding: Padding,
    subinfo: Option<Subinfo>,
}

impl<'a> PercentDisplay<'a> {
    pub fn new(range: impl Into<RangeInclusive<f32>>, value: impl Into<f32>) -> Self {
        let range: RangeInclusive<f32> = range.into();
        Self {
            value: value.into().clamp(*range.start(), *range.end()),
            range,
            size: 60.0,
            label: "".into(),
            display_type: DisplayType::Circle,
            padding: Padding::new(0.0),
            subinfo: None,
        }
    }
}

impl<'a, Subinfo> PercentDisplay<'a, Subinfo> {
    pub fn label(mut self, label: impl Into<Cow<'a, str>>) -> Self {
        self.label = label.into();
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn value(mut self, value: f32) -> Self {
        self.value = value.clamp(*self.range.start(), *self.range.end());
        self
    }

    pub fn range(mut self, range: RangeInclusive<f32>) -> Self {
        self.range = range;
        self
    }

    pub fn circle(mut self) -> Self {
        self.display_type = DisplayType::Circle;
        self
    }

    pub fn bar(mut self) -> Self {
        self.display_type = DisplayType::Bar;
        self
    }

    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    pub fn subinfo<NewSubinfo>(self, subinfo: NewSubinfo) -> PercentDisplay<'a, NewSubinfo> {
        self.maybe_subinfo(Some(subinfo))
    }

    /// Replaces the subinfo; passing `None` clears it.
    pub fn maybe_subinfo<NewSubinfo>(
        self,
        subinfo: Option<NewSubinfo>,
    ) -> PercentDisplay<'a, NewSubinfo> {
        PercentDisplay {
            range: self.range,
            value: self.value,
            size: self.size,
            label: self.label,
            display_type: self.display_type,
            padding: self.padding,
            subinfo,
        }
    }
}

impl<'a, Subinfo> PercentDisplay<'a, Subinfo> {
    pub fn into_widget<Message: Clone + 'static>(self) -> impl iced::Widget<Message> + 'a
    where
        Subinfo: iced::Widget<Message> + 'a,
    {
        let percent_display = self;
        let range = percent_display.range;
        let percent =
            (percent_display.value - *range.start()) / (*range.end() - *range.start()) * 100.0;
        match percent_display.display_type {
            DisplayType::Circle => column![
                stack![
                    progress_circle(range, percent_display.value).size(percent_display.size),
                    text!("{:.0}%", percent)
                        .center()
                        .width(Length::Fill)
                        .height(Length::Fill)
                ],
                if percent_display.label.len() > 0 {
                    Some(text(percent_display.label))
                } else {
                    None
                },
                percent_display.subinfo
            ]
            .padding(percent_display.padding)
            .align_x(alignment::Horizontal::Center)
            .into(),

            DisplayType::Bar => column![
                container(text(percent_display.label)),
                stack![
                    progress_bar(range, percent_display.value),
                    container(text!("{:.0}%", percent))
                        .padding(padding::left(20))
                        .align_y(Vertical::Center)
                        .height(Length::Fill)
                ],
                percent_display.subinfo
            ]
            .spacing(10)
            .padding(percent_display.padding)
            .into(),
        }
    }
}

pub enum DisplayType {
    Circle,
    Bar,
}
