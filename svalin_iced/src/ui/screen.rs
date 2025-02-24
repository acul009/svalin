use iced::{Subscription, Task};

use crate::Element;

use super::action::Action;

pub trait SubScreen {
    type Instruction;
    type Message;

    fn update(&mut self, message: Self::Message) -> Action<Self::Instruction, Self::Message>;

    fn view(&self) -> Element<Self::Message>;

    fn header(&self) -> Option<Element<Self::Message>> {
        None
    }

    fn dialog(&self) -> Option<Element<Self::Message>> {
        None
    }

    fn context(&self) -> Option<Element<Self::Message>> {
        None
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        Subscription::none()
    }
}
