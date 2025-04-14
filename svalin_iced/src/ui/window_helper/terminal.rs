use svalin::shared::commands::terminal::RemoteTerminal;

use crate::Element;

#[derive(Debug, Clone)]
pub enum Message {
    Terminal(frozen_term::Message),
}

pub struct TerminalWindow {
    title: String,
    term_display: frozen_term::Terminal,
    remote: RemoteTerminal,
}

impl TerminalWindow {
    pub fn new(remote: RemoteTerminal) -> Self {
        let (reader, writer) = tokio::io::simplex(2048);

        Self {
            title: String::from("Terminal"),
            term_display: frozen_term::Terminal::new(rows, cols, Box::new(writer)),
            remote,
        }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Terminal(message) => {
                let action = self.term_display.update(message);
                match action {
                    frozen_term::Action::None => {}
                    frozen_term::Action::Resize(size) => {
                        todo!()
                    }
                    frozen_term::Action::UpdateTitle(title) => {
                        self.title = title;
                    }
                }
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        todo!()
    }

    pub fn title(&self) -> String {
        self.title.clone()
    }
}
