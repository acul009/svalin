use iced::{advanced::subscription::from_recipe, widget::column};
use svalin::client::device::{Device, RemoteData};
use svalin_sysctl::realtime::RealtimeStatus;

use crate::{ui::widgets::realtime, util::watch_recipe::WatchRecipe};

#[derive(Debug, Clone)]
pub enum Message {
    Refresh,
}

pub struct DeviceStatus {
    recipe: WatchRecipe<String, RemoteData<RealtimeStatus>, Message>,
    realtime: RemoteData<RealtimeStatus>,
}

impl DeviceStatus {
    pub fn new(device: &Device) -> Self {
        Self {
            realtime: RemoteData::Pending,
            recipe: WatchRecipe::new(
                format!(
                    "realtime-{:x?}",
                    device.item().public_data.cert.fingerprint()
                ),
                device.subscribe_realtime(),
                Message::Refresh,
            ),
        }
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Refresh => self.realtime = self.recipe.borrow().clone(),
        }
    }

    pub fn view(&self) -> crate::Element<Message> {
        column![realtime(&self.realtime)].into()
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        from_recipe(self.recipe.clone())
    }
}
