use iced::{Subscription, Task};

use crate::Element;

pub trait SubScreen {
    type Message;

    fn update(&mut self, message: Self::Message) -> Task<Self::Message>;

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
