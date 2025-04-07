// Generated automatically by iced_fontello at build time.
// Do not edit manually. Source: ../../../fonts/icons.toml
// 5129f746b807449d7330c226e35fb4ec461be62ee54c6b97cffc5ed5e1212e4a
use iced::widget::{text, Text};
use iced::Font;

pub const FONT: &[u8] = include_bytes!("../../../fonts/icons.ttf");

pub fn add<'a>() -> Text<'a> {
    icon("\u{2B}")
}

pub fn back<'a>() -> Text<'a> {
    icon("\u{E75D}")
}

pub fn delete<'a>() -> Text<'a> {
    icon("\u{F1F8}")
}

pub fn device<'a>() -> Text<'a> {
    icon("\u{F108}")
}

pub fn edit<'a>() -> Text<'a> {
    icon("\u{270E}")
}

pub fn save<'a>() -> Text<'a> {
    icon("\u{1F4BE}")
}

pub fn tunnel<'a>() -> Text<'a> {
    icon("\u{21C6}")
}

fn icon(codepoint: &str) -> Text<'_> {
    text(codepoint).font(Font::with_name("icons"))
}
