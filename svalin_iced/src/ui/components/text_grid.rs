use std::borrow::Cow;

use crate::Element;

pub struct TextGrid {
    grid: Vec<char>,
    width: usize,
    height: usize,
}

impl TextGrid {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            grid: vec![' '; width * height],
            width,
            height,
        }
    }

    pub fn rows(&self) -> impl Iterator<Item = impl Into<Cow<'_, str>>> {
        self.grid
            .chunks(self.width)
            .map(|row| row.iter().collect::<String>())
    }

    pub fn view<Message>() -> Element<Message> {
        todo!()
    }
}
