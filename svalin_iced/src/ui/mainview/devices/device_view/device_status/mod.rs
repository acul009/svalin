use futures_util::SinkExt;
use iced::{Subscription, Task, stream::channel, widget::column};
use svalin::client::device::{Device, RemoteLiveData};
use svalin_sysctl::realtime::RealtimeStatus;

use crate::ui::{action::Action, screen::SubScreen, widgets::realtime};

#[derive(Debug, Clone)]
pub enum Message {
    Realtime(RemoteLiveData<RealtimeStatus>),
}

pub enum Instruction {}

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
}

impl SubScreen for DeviceStatus {
    type Instruction = Instruction;
    type Message = Message;

    fn update(&mut self, message: Self::Message) -> Action<Instruction, Message> {
        match message {
            Message::Realtime(remote_live_data) => {
                self.realtime = remote_live_data;
                Action::none()
            }
        }
    }

    fn view(&self) -> crate::Element<Self::Message> {
        column![realtime(&self.realtime)].into()
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
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
