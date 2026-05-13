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
use svalin_pki::SpkiHash;
use tokio::sync::broadcast;

use crate::ui::{
    mainview::device_view::DeviceView,
    widgets::{error_display, loading},
};

mod add_device;
mod device_list;
mod device_view;

#[derive(Debug, Clone)]
pub enum Message {
    InitState(Arc<(ClientState, broadcast::Receiver<ClientStateUpdate>)>),
    Error(Arc<anyhow::Error>),
    UpdateState(ClientStateUpdate),
    OpenAddDevice,
    SelectDevice(SpkiHash),
    AddDevice(add_device::Message),
    DeviceView(device_view::Message),
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
    DeviceView(device_view::DeviceView),
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
                        while let Ok(update) = receiver.recv().await {
                            tracing::debug!("got state update: {update:?}");
                            sender.send(Message::UpdateState(update)).await;
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
            Message::SelectDevice(spki_hash) => {
                self.screen = Screen::DeviceView(DeviceView::new(spki_hash));
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
                    add_device::Action::Close => {
                        self.screen = Screen::DeviceList;
                        Action::None
                    }
                    add_device::Action::Done(spki_hash) => {
                        self.screen = Screen::DeviceView(DeviceView::new(spki_hash));
                        Action::None
                    }
                    add_device::Action::Run(task) => Action::Run(task.map(Message::AddDevice)),
                }
            }
            Message::DeviceView(message) => {
                let Screen::DeviceView(device_view) = &mut self.screen else {
                    return Action::None;
                };

                match device_view.update(message) {
                    device_view::Action::None => Action::None,
                    device_view::Action::Back => {
                        self.screen = Screen::DeviceList;
                        Action::None
                    }
                    device_view::Action::Run(task) => Action::Run(task.map(Message::DeviceView)),
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
                .on_select(Message::SelectDevice)
                .into(),
            Screen::AddDevice(add_device) => add_device.view().map(Message::AddDevice),
            Screen::DeviceView(device_view) => {
                device_view.view(&self.state).map(Message::DeviceView)
            }
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

    pub(crate) fn shutdown(self) -> Task<()> {
        let client = self.client;

        Task::future(async move {
            if let Err(err) = client.close(Duration::from_secs(3)).await {
                tracing::error!("{err:#}");
                process::exit(1);
            }
        })
    }
}
