use iced::{Subscription, Task};
use iced_term::TerminalView;
use svalin::{client::device::Device, shared::commands::terminal::RemoteTerminal};

use crate::ui::screen::SubScreen;

pub enum Message {
    Terminal(iced_term::Event),
}

pub struct Terminal {
    terminal_connection: RemoteTerminal,
    terminal_display: iced_term::Terminal,
}

impl Terminal {
    pub fn start(terminal: RemoteTerminal) -> (Self, Task<Message>) {
        (
            Self {
                terminal_connection: terminal,
                terminal_display: iced_term::Terminal::new(
                    0,
                    iced_term::settings::Settings::default(),
                ),
            },
            Task::none(),
        )
    }
}

impl SubScreen for Terminal {
    type Message = Message;

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::Terminal(iced_term::Event::CommandReceived(_, cmd)) => {
                self.terminal_display.update(cmd);
                Task::none()
            }
        }
    }

    fn view(&self) -> crate::Element<Self::Message> {
        TerminalView::show(&self.terminal_display).map(Message::Terminal)
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        let term_subscription = iced_term::Subscription::new(self.terminal_display.id);
        let term_event_stream = term_subscription.event_stream();
        Subscription::run_with_id(self.terminal_display.id, term_event_stream)
            .map(Message::Terminal)
    }
}
