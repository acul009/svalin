use iced::Widget;

#[derive(Debug, Clone)]
pub enum Message {}

pub struct SessionsView;

impl SessionsView {
    pub fn new() -> Self {
        Self
    }

    pub fn update(&mut self, _message: Message) {}

    pub fn view(&self) -> impl Widget<Message> + '_ {
        iced::widget::void()
    }
}

pub fn view(state: &SessionsView) -> impl Widget<Message> + '_ {
    state.view()
}
