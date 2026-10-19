use std::{process, sync::Arc, time::Duration};

use iced::{
    Subscription, Task, Widget, keyboard,
    widget::{column, row, rule, scrollable, stack, text, text_input},
};
use svalin::client::{
    Client,
    state::{ClientState, Update, warning},
};
use svalin_pki::SpkiHash;
use tokio::sync::broadcast;
use tokio_stream::{StreamExt, wrappers::BroadcastStream};

use crate::{
    bootstrap,
    ui::{
        ERROR_COLOR, INFO_COLOR, WARNING_COLOR,
        widgets::{button_list, error_display, header, icon_button, loading},
    },
};

mod add_device;
mod device_list;
mod device_view;
mod server_view;
mod tunnel_menu;
mod warning_menu;

#[derive(Debug, Clone)]
pub enum Message {
    InitState(Arc<(ClientState, broadcast::Receiver<Update>)>),
    Error(Arc<anyhow::Error>),
    UpdateState(Update),
    OpenAddDevice,
    SelectDevice(SpkiHash),
    AddDevice(add_device::Message),
    DeviceView(device_view::Message),
    TunnelMenu(tunnel_menu::Message),
    OpenTerminal(
        Arc<(
            tokio::sync::mpsc::Sender<async_pty::TerminalInput>,
            tokio::sync::mpsc::Receiver<Vec<u8>>,
        )>,
    ),
    CloseError,
    Context(Context),
    GroupFilter(GroupFilter),
    Search(String),
    FocusSearch,
    Section(Section),
    ServerView(server_view::Message),
}

pub enum Action {
    None,
    Run(Task<Message>),
    OpenTerminal(
        Arc<(
            tokio::sync::mpsc::Sender<async_pty::TerminalInput>,
            tokio::sync::mpsc::Receiver<Vec<u8>>,
        )>,
    ),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Devices,
    Server,
}

enum Screen {
    Loading(String),
    DeviceList,
    AddDevice(add_device::AddDevice),
    DeviceView(device_view::State),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Context {
    None,
    Warnings,
    Tunnels,
}

impl From<Context> for Message {
    fn from(context: Context) -> Self {
        Self::Context(context)
    }
}

#[derive(Debug, Clone)]
pub enum GroupFilter {
    Group(Option<String>),
    NoFilter,
}

impl From<&Option<String>> for GroupFilter {
    fn from(value: &Option<String>) -> Self {
        GroupFilter::Group(value.clone())
    }
}

pub struct MainView {
    screen: Screen,
    section: Section,
    server_view: server_view::ServerView,
    state: ClientState,
    context: Context,
    client: Arc<Client>,
    error: Option<Arc<anyhow::Error>>,
    update_abort_handle: Option<iced::task::Handle>,
    group_filter: GroupFilter,
    search: String,
}

impl MainView {
    pub fn new(client: Arc<Client>) -> (Self, Task<Message>) {
        let client2 = client.clone();
        (
            Self {
                section: Section::Devices,
                server_view: server_view::ServerView::new(),
                screen: Screen::Loading(t!("device-list.loading").to_string()),
                state: ClientState::empty(),
                context: Context::None,
                client,
                error: None,
                update_abort_handle: None,
                group_filter: GroupFilter::NoFilter,
                search: String::new(),
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
            Message::CloseError => {
                self.error = None;
                Action::None
            }
            Message::InitState(state) => {
                let (state, receiver) = Arc::into_inner(state).unwrap();
                self.state = state;
                self.screen = Screen::DeviceList;

                let updates = BroadcastStream::new(receiver)
                    .map_while(|update| update.ok().map(Message::UpdateState));
                let (update_task, abort_handle) = Task::stream(updates).abortable();
                self.update_abort_handle = Some(abort_handle.abort_on_drop());

                Action::Run(update_task)
            }
            Message::UpdateState(update) => {
                tracing::trace!("got update: {update:?}");
                self.state.update(update);
                Action::None
            }
            Message::SelectDevice(spki_hash) => {
                self.section = Section::Devices;
                self.screen = Screen::DeviceView(device_view::State::new(spki_hash));
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
                        self.screen = Screen::DeviceView(device_view::State::new(spki_hash));
                        Action::None
                    }
                    add_device::Action::Run(task) => Action::Run(task.map(Message::AddDevice)),
                }
            }
            Message::DeviceView(message) => {
                let Screen::DeviceView(device_view) = &mut self.screen else {
                    return Action::None;
                };
                let action = device_view.update(message, &self.state, &self.client);
                match action {
                    device_view::Action::None => Action::None,
                    device_view::Action::Error(error) => {
                        self.error = Some(error);
                        Action::None
                    }
                    device_view::Action::Back => {
                        self.screen = Screen::DeviceList;
                        Action::None
                    }
                    device_view::Action::OpenTerminal(spki_hash) => {
                        let client = self.client.clone();
                        Action::Run(Task::future(async move {
                            // TODO: Handle error
                            Message::OpenTerminal(Arc::new(
                                client.device(spki_hash).open_terminal().await.unwrap(),
                            ))
                        }))
                    }
                    device_view::Action::Run(task) => Action::Run(task.map(Message::DeviceView)),
                }
            }
            Message::OpenTerminal(arc) => Action::OpenTerminal(arc),
            Message::TunnelMenu(message) => match tunnel_menu::update(message, &self.client) {
                tunnel_menu::Action::None => Action::None,
                tunnel_menu::Action::SelectDevice(device) => {
                    self.section = Section::Devices;
                    self.screen = Screen::DeviceView(device_view::State::new(device));
                    Action::None
                }
            },
            Message::Context(context) => {
                self.context = if self.context == context {
                    Context::None
                } else {
                    context
                };
                Action::None
            }
            Message::GroupFilter(filter) => {
                self.group_filter = filter;
                Action::None
            }
            Message::Search(search) => {
                self.search = search;
                Action::None
            }
            Message::Section(section) => {
                self.section = section;
                match section {
                    Section::Devices => Action::None,
                    Section::Server => Action::Run(
                        self.server_view
                            .update(
                                server_view::Message::SelectScreen(server_view::Screen::Sessions),
                                &self.client,
                            )
                            .map(Message::ServerView),
                    ),
                }
            }
            Message::ServerView(message) => Action::Run(
                self.server_view
                    .update(message, &self.client)
                    .map(Message::ServerView),
            ),
            Message::FocusSearch => {
                Action::Run(iced::widget::operation::focus("device-search").chain(
                    iced::widget::operation::text_input::select_all("device-search"),
                ))
            }
        }
    }

    pub fn view(&self) -> impl Widget<Message> + '_ {
        let screen = match self.section {
            Section::Devices => self.devices_view(),
            Section::Server => self.server_view.view().map(Message::ServerView).boxed(),
        };

        stack![
            screen,
            self.error.as_ref().map(|error| {
                error_display(error)
                    .on_close(Message::CloseError)
                    .into_overlay()
            }),
        ]
    }

    fn devices_view(&self) -> crate::Element<'_, Message> {
        match &self.screen {
            Screen::Loading(text) => loading(text).into_widget().boxed(),
            Screen::DeviceList => column![
                row![
                    bootstrap::search().size(20),
                    text_input(t!("device-list.search"), &self.search)
                        .id("device-search")
                        .on_input(Message::Search),
                    icon_button(bootstrap::x_lg())
                        .size(32)
                        .tooltip(text(t!("device-list.clear-search")))
                        .on_press(Message::Search(String::new()))
                        .into_widget(),
                ]
                .align_y(iced::alignment::Vertical::Center)
                .spacing(10)
                .padding([8, 10]),
                device_list::DeviceList::new(&self.state)
                    .on_new(Message::OpenAddDevice)
                    .on_select(Message::SelectDevice)
                    .group_filter(&self.group_filter)
                    .search(&self.search)
                    .into_widget(),
            ]
            .boxed(),
            Screen::AddDevice(add_device) => stack![
                device_list::DeviceList::new(&self.state).into_widget(),
                add_device.view().map(Message::AddDevice)
            ]
            .boxed(),
            Screen::DeviceView(device_view) => device_view
                .view(&self.state)
                .map(Message::DeviceView)
                .boxed(),
        }
    }

    pub fn rail(&self) -> impl Widget<Message> + '_ {
        use crate::ui::widgets::rail::{Item, Rail};

        Rail::new()
            .items([Item::new(bootstrap::pc_display())
                .tooltip(text(t!("navigation.devices")))
                .selected(self.section == Section::Devices)
                .on_press(Message::Section(Section::Devices))])
            .bottom_items([Item::new(bootstrap::server())
                .tooltip(text(t!("navigation.server")))
                .selected(self.section == Section::Server)
                .on_press(Message::Section(Section::Server))])
            .into_widget()
    }

    pub fn header(&self) -> header::Header<'_, Message> {
        let header = match self.section {
            Section::Server => header(text(t!("navigation.server"))),
            Section::Devices => match &self.screen {
                Screen::DeviceView(device_view) => {
                    device_view.header(&self.state).map(Message::DeviceView)
                }
                _ => header(iced::widget::void()),
            },
        };

        let severity = self.state.warnings().highest_severity();

        header
            .action(
                icon_button(bootstrap::ethernet())
                    .selected(self.context == Context::Tunnels)
                    .tooltip(text(t!("tunnel.menu.show")))
                    .on_press(Context::Tunnels.into())
                    .into_widget(),
            )
            .action(
                icon_button(
                    bootstrap::exclamation_triangle_fill().color_maybe(severity.map(|severity| {
                        match severity {
                            warning::Severity::High => ERROR_COLOR,
                            warning::Severity::Medium => WARNING_COLOR,
                            warning::Severity::Low => INFO_COLOR,
                        }
                    })),
                )
                .selected(self.context == Context::Warnings)
                .tooltip(text(t!("warnings.show")))
                .on_press(Context::Warnings.into())
                .into_widget(),
            )
    }

    pub fn context(&self) -> Option<crate::Element<'_, Message>> {
        match &self.context {
            Context::None => None,
            Context::Tunnels => Some(
                column![
                    tunnel_menu::header()
                        .on_back(Context::None.into())
                        .into_widget(),
                    tunnel_menu::view(&self.state).map(Message::TunnelMenu),
                ]
                .boxed(),
            ),
            Context::Warnings => Some(
                column![
                    warning_menu::header()
                        .on_back(Context::None.into())
                        .into_widget(),
                    warning_menu::view(&self.state)
                ]
                .boxed(),
            ),
        }
    }

    pub(crate) fn sidebar(&self) -> Option<crate::Element<'_, Message>> {
        match self.section {
            Section::Server => Some(self.server_view.sidebar().map(Message::ServerView).boxed()),
            Section::Devices => self.devices_sidebar(),
        }
    }

    fn devices_sidebar(&self) -> Option<crate::Element<'_, Message>> {
        match &self.screen {
            Screen::DeviceList => Some(
                column![
                    button_list::entry(text(t!("device-list.all")))
                        .selected(matches!(self.group_filter, GroupFilter::NoFilter))
                        .on_press(Message::GroupFilter(GroupFilter::NoFilter))
                        .into_widget(40, [8, 16], false),
                    rule::horizontal(2),
                    scrollable(
                        button_list(self.state.groups().map(|group| {
                            let name: std::borrow::Cow<'_, str> = group
                                .as_ref()
                                .map(|s| s.as_str().into())
                                .unwrap_or_else(|| t!("device-list.nogroup"));
                            button_list::entry(text(name))
                                .selected(matches!(&self.group_filter, GroupFilter::Group(selected) if selected == group))
                                .on_press(Message::GroupFilter(group.into()))
                        }))
                        .entry_height(40)
                        .into_widget()
                    )
                    .height(iced::Length::Fill)
                ]
                .boxed(),
            ),
            _ => None,
        }
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        match &self.screen {
            Screen::DeviceList if self.error.is_none() && self.section == Section::Devices => {
                keyboard::listen().filter_map(|event| match event {
                    keyboard::Event::KeyPressed {
                        key: keyboard::Key::Character(key),
                        modifiers,
                        ..
                    } if modifiers.control() && key.eq_ignore_ascii_case("f") => {
                        Some(Message::FocusSearch)
                    }
                    _ => None,
                })
            }
            _ => Subscription::none(),
        }
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
