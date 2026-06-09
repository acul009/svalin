use iced::widget::{button, column, row, space, text, text_editor, text_input};
use svalin_client_store::persistent::SvalinMetaInfo;
use svalin_pki::get_current_timestamp;

use crate::{bootstrap, ui::widgets::card};

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

    pub fn update<'a>(
        &'a mut self,
        msg: Message,
        current_info: &'a SvalinMetaInfo,
    ) -> Option<SvalinMetaInfo> {
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
                return Some(SvalinMetaInfo {
                    name: self.name.clone(),
                    group: self.group.clone(),
                    notes: self.notes.text(),
                    updated_at: get_current_timestamp(),
                });
            }
        }

        None
    }

    pub fn view<'a>(&'a self, current_info: &'a SvalinMetaInfo) -> crate::Element<'a, Message> {
        if self.edit {
            card(
                column![
                    row![
                        "Name:",
                        space::horizontal(),
                        text_input("", &self.name).on_input(Message::ChangeName)
                    ],
                    row![
                        "Group:",
                        space::horizontal(),
                        text_input("", &self.group).on_input(Message::ChangeGroup)
                    ],
                    row![
                        "Notes:",
                        space::horizontal(),
                        text_editor(&self.notes).on_action(Message::ChangeNotes)
                    ],
                ]
                .spacing(10),
            )
            .title(row![
                "Device Information",
                space::horizontal(),
                button(bootstrap::floppy()).on_press(Message::Save),
                button(bootstrap::x_square()).on_press(Message::CancelEdit)
            ])
            .into()
        } else {
            card(
                column![
                    row!["Name:", space::horizontal(), text(&current_info.name)],
                    row!["Group:", space::horizontal(), text(&current_info.group)],
                    row!["Notes:", space::horizontal(), text(&current_info.notes)],
                ]
                .spacing(10),
            )
            .title(row![
                "Device Information",
                space::horizontal(),
                button(bootstrap::pencil()).on_press(Message::Edit)
            ])
            .into()
        }
    }
}
