use std::{borrow::Cow, ops::RangeInclusive};

use iced::{
    alignment::{self, Vertical},
    padding,
    widget::{column, container, progress_bar, row, stack, text},
    Length, Padding,
};

use crate::Element;

use super::progress_circle;

pub struct PercentDisplay<'a, Message> {
    range: RangeInclusive<f32>,
    value: f32,
    size: f32,
    label: Cow<'a, str>,
    display_type: DisplayType,
    padding: Padding,
    subinfo: Option<Element<'a, Message>>,
}

impl<'a, Message> PercentDisplay<'a, Message> {
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

    pub fn subinfo(mut self, subinfo: impl Into<Element<'a, Message>>) -> Self {
        self.subinfo = Some(subinfo.into());
        self
    }

    pub fn maybe_subinfo(self, subinfo: Option<impl Into<Element<'a, Message>>>) -> Self {
        if let Some(subinfo) = subinfo {
            return self.subinfo(subinfo);
        };
        self
    }
}

impl<'a, Message: Clone + 'static> From<PercentDisplay<'a, Message>> for Element<'a, Message> {
    fn from(percent_display: PercentDisplay<'a, Message>) -> Self {
        let range = percent_display.range;
        let percent =
            (percent_display.value - *range.start()) / (*range.end() - *range.start()) * 100.0;
        match percent_display.display_type {
            DisplayType::Circle => column![stack![
                progress_circle(range, percent_display.value).size(percent_display.size),
                text!("{:.0}%", percent)
                    .center()
                    .width(Length::Fill)
                    .height(Length::Fill)
            ]]
            .push_maybe(if percent_display.label.len() > 0 {
                Some(text(percent_display.label))
            } else {
                None
            })
            .push_maybe(percent_display.subinfo)
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
                ]
            ]
            .push_maybe(percent_display.subinfo)
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
