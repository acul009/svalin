use cosmic::{
    iced_widget::{button, column, row, text},
    Element,
};

#[derive(Debug, Clone)]
pub enum Message {
    IncrementCounter,
    DecrementCounter,
}

pub struct Counter {
    count: i32,
}

impl Counter {
    pub fn new() -> Counter {
        Counter { count: 0 }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::IncrementCounter => self.count += 1,
            Message::DecrementCounter => self.count -= 1,
        }
    }

    pub fn view(&self) -> Element<Message> {
        column![
            text(format!("{}", self.count)),
            row![
                button(text("Decrement")).on_press(Message::DecrementCounter),
                button(text("Increment")).on_press(Message::IncrementCounter)
            ]
        ]
        .into()
    }
}
