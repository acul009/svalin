use std::{process, sync::Arc, time::Duration};

use iced::{
    Subscription, Task,
    task::sipper,
    widget::{self},
};
use svalin::client::{
    Client,
    state::{ClientState, ClientStateUpdate},
};
use tokio::sync::broadcast;

use crate::ui::widgets::{error_display, loading};

mod add_device;
mod device_list;

#[derive(Debug, Clone)]
pub enum Message {
    InitState(Arc<(ClientState, broadcast::Receiver<ClientStateUpdate>)>),
    Error(Arc<anyhow::Error>),
    UpdateState(ClientStateUpdate),
    OpenAddDevice,
    AddDevice(add_device::Message),
}

pub enum Action {
    None,
    Run(Task<Message>),
    // OpenTerminal(Device),
}

enum Screen {
    Loading(String),
    DeviceList,
    AddDevice(add_device::AddDevice),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Context {
    None,
}

pub struct MainView {
    screen: Screen,
    state: ClientState,
    context: Context,
    client: Arc<Client>,
    error: Option<Arc<anyhow::Error>>,
    update_abort_handle: Option<iced::task::Handle>,
}

impl MainView {
    pub fn new(client: Arc<Client>) -> (Self, Task<Message>) {
        let client2 = client.clone();
        (
            Self {
                screen: Screen::Loading("Loading devices...".into()),
                state: ClientState::empty(),
                context: Context::None,
                client,
                error: None,
                update_abort_handle: None,
            },
            Task::future(async move {
                match client2.subscribe_state().await {
                    Ok(state) => Message::InitState(Arc::new(state)),
                    Err(error) => Message::Error(Arc::new(error)),
                }
            }),
        )
    }

    #[must_use]
    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Error(error) => {
                self.error = Some(error);
                Action::None
            }
            Message::InitState(state) => {
                let (state, receiver) = Arc::into_inner(state).unwrap();
                self.state = state;
                self.screen = Screen::DeviceList;

                let (update_task, abort_handle) =
                    Task::stream(sipper(move |mut sender| async move {
                        let mut receiver = receiver;
                        while let Ok(state) = receiver.recv().await {
                            sender.send(Message::UpdateState(state)).await;
                        }
                    }))
                    .abortable();
                self.update_abort_handle = Some(abort_handle.abort_on_drop());

                Action::Run(update_task)
            }
            Message::UpdateState(update) => {
                self.state.update(update);
                Action::None
            }
            Message::OpenAddDevice => {
                let (add_device, task) = add_device::AddDevice::new();
                self.screen = Screen::AddDevice(add_device);
                Action::Run(task.map(Message::AddDevice))
            }
            Message::AddDevice(message) => {
                let Screen::AddDevice(add_device) = &mut self.screen else {
                    return Action::None;
                };

                match add_device.update(message, &self.client) {
                    add_device::Action::None => Action::None,
                    add_device::Action::Close | add_device::Action::Done(_) => {
                        self.screen = Screen::DeviceList;
                        Action::None
                    }
                    add_device::Action::Run(task) => Action::Run(task.map(Message::AddDevice)),
                }
            }
        }
    }

    pub fn view(&self) -> crate::Element<'_, Message> {
        if let Some(error) = &self.error {
            return error_display(error).into();
        }

        match &self.screen {
            Screen::Loading(text) => loading(text).into(),
            Screen::DeviceList => device_list::DeviceList::new(&self.state)
                .on_new(Message::OpenAddDevice)
                .into(),
            Screen::AddDevice(add_device) => add_device.view().map(Message::AddDevice),
        }
    }

    pub fn header(&self) -> crate::Element<'_, Message> {
        widget::space().into()
    }

    pub fn context(&self) -> Option<crate::Element<'_, Message>> {
        match &self.context {
            Context::None => None,
            // Context::Tunnel => Some(self.tunnel_ui.view().map(Message::Tunnel)),
            // Context::Test => Some(text("test").into()),
        }
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        // let state_subscription = match &self.state {
        //     Screen::DeviceList => self.devices.subscription().map(Message::Devices),
        // };

        // let context_subscription = match &self.context {
        //     Context::None => None,
        //     Context::Tunnel => Some(self.tunnel_ui.subscription().map(Message::Tunnel)),
        //     Context::Test => None,
        // };

        // let mut subscriptions = vec![state_subscription];

        // if let Some(context_subscription) = context_subscription {
        //     subscriptions.push(context_subscription);
        // }

        // Subscription::batch(subscriptions)
        Subscription::none()
    }

    pub(crate) fn shutdown(&mut self) -> Task<Message> {
        self.screen = Screen::Loading("Shutting down...".to_string());
        let client = self.client.clone();

        Task::future(async move {
            if let Err(err) = client.close(Duration::from_secs(3)).await {
                tracing::error!("{err:#}");
                process::exit(1);
            }
        })
        .then(|_| iced::exit())
    }
}
