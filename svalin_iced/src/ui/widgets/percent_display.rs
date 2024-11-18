use std::{borrow::Cow, ops::RangeInclusive};

use iced::{
    alignment,
    widget::{column, text},
    Padding,
};

use crate::Element;

use super::progress_circle;

pub struct PercentDisplay<'a> {
    range: RangeInclusive<f32>,
    value: f32,
    size: f32,
    label: Cow<'a, str>,
    display_type: DisplayType,
    padding: Padding,
}

impl<'a> PercentDisplay<'a> {
    pub fn new(range: RangeInclusive<f32>, value: f32) -> Self {
        Self {
            value: value.clamp(*range.start(), *range.end()),
            range,
            size: 40.0,
            label: "".into(),
            display_type: DisplayType::Circle,
            padding: Padding::new(0.0),
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
}

impl<'a, Message: Clone + 'static> From<PercentDisplay<'a>> for Element<'a, Message> {
    fn from(value: PercentDisplay<'a>) -> Self {
        let range = value.range;
        match value.display_type {
            DisplayType::Circle => column![
                progress_circle(range.clone(), value.value).size(value.size),
                text!(
                    "{:.0}%",
                    (value.value - *range.start()) / (*range.end() - *range.start())
                )
                .center(),
            ]
            .padding(value.padding)
            .align_x(alignment::Horizontal::Center)
            .into(),
            DisplayType::Bar => {
                todo!()
            }
        }
    }
}

pub enum DisplayType {
    Circle,
    Bar,
}
