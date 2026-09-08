use iced::widget::{column, row, space, text, text_editor, text_input};
use svalin::client::state::persistent::MetaInfo;
use svalin_pki::get_current_timestamp;

use crate::{
    bootstrap,
    ui::widgets::{card, icon_button},
};

#[derive(Debug, Clone)]
pub enum Message {
    Edit,
    CancelEdit,
    ChangeName(String),
    ChangeGroup(String),
    ChangeNotes(text_editor::Action),
    Save,
}

pub struct State {
    edit: bool,
    name: String,
    group: String,
    notes: text_editor::Content,
}

impl State {
    pub fn new() -> Self {
        Self {
            edit: false,
            name: String::new(),
            group: String::new(),
            notes: text_editor::Content::new(),
        }
    }

    pub fn update<'a>(&'a mut self, msg: Message, current_info: &'a MetaInfo) -> Option<MetaInfo> {
        match msg {
            Message::Edit => {
                self.edit = true;
                self.name = current_info.name.clone();
                self.group = current_info.group.clone();
                self.notes = text_editor::Content::with_text(&current_info.notes);
            }
            Message::CancelEdit => self.edit = false,
            Message::ChangeName(name) => self.name = name,
            Message::ChangeGroup(group) => self.group = group,
            Message::ChangeNotes(action) => {
                self.notes.perform(action);
            }
            Message::Save => {
                self.edit = false;
                return Some(MetaInfo {
                    name: self.name.clone(),
                    group: self.group.clone(),
                    notes: self.notes.text(),
                    updated_at: get_current_timestamp(),
                });
            }
        }

        None
    }

    pub fn view<'a>(&'a self, current_info: &'a MetaInfo) -> crate::Element<'a, Message> {
        if self.edit {
            card(
                column![
                    row![
                        text(t!("device.info.name")),
                        space::horizontal(),
                        text_input("", &self.name).on_input(Message::ChangeName)
                    ],
                    row![
                        text(t!("device.info.group")),
                        space::horizontal(),
                        text_input("", &self.group).on_input(Message::ChangeGroup)
                    ],
                    row![
                        text(t!("device.info.notes")),
                        space::horizontal(),
                        text_editor(&self.notes).on_action(Message::ChangeNotes)
                    ],
                ]
                .spacing(10),
            )
            .title(text(t!("device.info.title")))
            .action(icon_button(bootstrap::floppy()).on_press(Message::Save))
            .action(icon_button(bootstrap::x_square()).on_press(Message::CancelEdit))
            .into()
        } else {
            card(
                column![
                    row![
                        text(t!("device.info.name")),
                        space::horizontal(),
                        text(&current_info.name)
                    ],
                    row![
                        text(t!("device.info.group")),
                        space::horizontal(),
                        text(&current_info.group)
                    ],
                    row![
                        text(t!("device.info.notes")),
                        space::horizontal(),
                        text(&current_info.notes)
                    ],
                ]
                .spacing(10),
            )
            .title(text(t!("device.info.title")))
            .action(icon_button(bootstrap::pencil()).on_press(Message::Edit))
            .into()
        }
    }
}
