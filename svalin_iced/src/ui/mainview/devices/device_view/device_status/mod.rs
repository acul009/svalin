use futures_util::SinkExt;
use iced::{Subscription, Task, stream::channel, widget::column};
use svalin::client::device::{Device, RemoteLiveData};
use svalin_sysctl::realtime::RealtimeStatus;

use crate::ui::widgets::realtime;

#[derive(Debug, Clone)]
pub enum Message {
    Realtime(RemoteLiveData<RealtimeStatus>),
}

pub struct DeviceStatus {
    device: Device,
    realtime: RemoteLiveData<RealtimeStatus>,
}

impl DeviceStatus {
    pub fn start(device: Device) -> (Self, Task<Message>) {
        (
            Self {
                device,
                realtime: RemoteLiveData::Pending,
            },
            Task::none(),
        )
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::Realtime(remote_live_data) => {
                self.realtime = remote_live_data;
            }
        }
    }

    pub fn view(&self) -> crate::Element<Message> {
        column![realtime(&self.realtime)].into()
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        let device = self.device.clone();
        Subscription::run_with_id(
            format!(
                "realtime-{:x?}",
                self.device.item().public_data.cert.fingerprint()
            ),
            channel(1, move |mut output| async move {
                let mut subscription = device.subscribe_realtime();

                output
                    .send(Message::Realtime(subscription.current_owned()))
                    .await
                    .unwrap();

                while let Ok(()) = subscription.changed().await {
                    output
                        .send(Message::Realtime(subscription.current_owned()))
                        .await
                        .unwrap();
                }
            }),
        )
    }
}
