use iced::{
    Color, Length, Shadow, Widget,
    widget::{column, container, row, rule, space, stack},
};

use iced::widget::Void;

pub struct Scaffold<
    Body,
    Header = Void,
    Footer = Void,
    Dialog = Void,
    Sidebar = Void,
    Context = Void,
    Rail = Void,
> {
    body: Body,
    header: Option<Header>,
    footer: Option<Footer>,
    dialog: Option<Dialog>,
    sidebar: Option<Sidebar>,
    context: Option<Context>,
    rail: Option<Rail>,
}

impl<Body> Scaffold<Body> {
    pub fn new(body: Body) -> Self {
        Self {
            body,
            header: None,
            footer: None,
            dialog: None,
            sidebar: None,
            context: None,
            rail: None,
        }
    }
}

impl<Body, Header, Footer, Dialog, Sidebar, Context, Rail>
    Scaffold<Body, Header, Footer, Dialog, Sidebar, Context, Rail>
{
    pub fn header<NewHeader>(
        self,
        header: NewHeader,
    ) -> Scaffold<Body, NewHeader, Footer, Dialog, Sidebar, Context, Rail> {
        self.header_maybe(Some(header))
    }

    /// Replaces the header; passing `None` clears it.
    pub fn header_maybe<NewHeader>(
        self,
        header: Option<NewHeader>,
    ) -> Scaffold<Body, NewHeader, Footer, Dialog, Sidebar, Context, Rail> {
        Scaffold {
            body: self.body,
            header,
            footer: self.footer,
            dialog: self.dialog,
            sidebar: self.sidebar,
            context: self.context,
            rail: self.rail,
        }
    }

    pub fn footer<NewFooter>(
        self,
        footer: NewFooter,
    ) -> Scaffold<Body, Header, NewFooter, Dialog, Sidebar, Context, Rail> {
        self.footer_maybe(Some(footer))
    }

    /// Replaces the footer; passing `None` clears it.
    pub fn footer_maybe<NewFooter>(
        self,
        footer: Option<NewFooter>,
    ) -> Scaffold<Body, Header, NewFooter, Dialog, Sidebar, Context, Rail> {
        Scaffold {
            body: self.body,
            header: self.header,
            footer,
            dialog: self.dialog,
            sidebar: self.sidebar,
            context: self.context,
            rail: self.rail,
        }
    }

    pub fn dialog<NewDialog>(
        self,
        dialog: NewDialog,
    ) -> Scaffold<Body, Header, Footer, NewDialog, Sidebar, Context, Rail> {
        self.dialog_maybe(Some(dialog))
    }

    /// Replaces the dialog; passing `None` clears it.
    pub fn dialog_maybe<NewDialog>(
        self,
        dialog: Option<NewDialog>,
    ) -> Scaffold<Body, Header, Footer, NewDialog, Sidebar, Context, Rail> {
        Scaffold {
            body: self.body,
            header: self.header,
            footer: self.footer,
            dialog,
            sidebar: self.sidebar,
            context: self.context,
            rail: self.rail,
        }
    }

    pub fn sidebar<NewSidebar>(
        self,
        sidebar: NewSidebar,
    ) -> Scaffold<Body, Header, Footer, Dialog, NewSidebar, Context, Rail> {
        self.sidebar_maybe(Some(sidebar))
    }

    /// Replaces the sidebar; passing `None` clears it.
    pub fn sidebar_maybe<NewSidebar>(
        self,
        sidebar: Option<NewSidebar>,
    ) -> Scaffold<Body, Header, Footer, Dialog, NewSidebar, Context, Rail> {
        Scaffold {
            body: self.body,
            header: self.header,
            footer: self.footer,
            dialog: self.dialog,
            sidebar,
            context: self.context,
            rail: self.rail,
        }
    }

    pub fn context<NewContext>(
        self,
        context: NewContext,
    ) -> Scaffold<Body, Header, Footer, Dialog, Sidebar, NewContext, Rail> {
        self.context_maybe(Some(context))
    }

    /// Replaces the context; passing `None` clears it.
    pub fn context_maybe<NewContext>(
        self,
        context: Option<NewContext>,
    ) -> Scaffold<Body, Header, Footer, Dialog, Sidebar, NewContext, Rail> {
        Scaffold {
            body: self.body,
            header: self.header,
            footer: self.footer,
            dialog: self.dialog,
            sidebar: self.sidebar,
            context,
            rail: self.rail,
        }
    }

    /// Adds a navigation rail to the left of the sidebar.
    pub fn rail<NewRail>(
        self,
        rail: NewRail,
    ) -> Scaffold<Body, Header, Footer, Dialog, Sidebar, Context, NewRail> {
        self.rail_maybe(Some(rail))
    }

    /// Replaces the rail; passing `None` clears it.
    pub fn rail_maybe<NewRail>(
        self,
        rail: Option<NewRail>,
    ) -> Scaffold<Body, Header, Footer, Dialog, Sidebar, Context, NewRail> {
        Scaffold {
            body: self.body,
            header: self.header,
            footer: self.footer,
            dialog: self.dialog,
            sidebar: self.sidebar,
            context: self.context,
            rail,
        }
    }

    pub fn into_widget<'a, Message: Clone + 'static>(self) -> impl iced::Widget<Message> + 'a
    where
        Body: iced::Widget<Message> + 'a,
        Header: iced::Widget<Message> + 'a,
        Footer: iced::Widget<Message> + 'a,
        Dialog: iced::Widget<Message> + 'a,
        Sidebar: iced::Widget<Message> + 'a,
        Context: iced::Widget<Message> + 'a,
        Rail: iced::Widget<Message> + 'a,
    {
        let scaffold = self;
        let has_header = scaffold.header.is_some();
        let has_context = scaffold.context.is_some();
        let has_rail = scaffold.rail.is_some();
        let has_sidebar = scaffold.sidebar.is_some();
        stack![
            column![
                scaffold.header.map(|header| {
                    container(header).style(|_| container::Style {
                        shadow: Shadow {
                            blur_radius: 12.0,
                            color: Color::BLACK.scale_alpha(0.4),
                            offset: iced::Vector { x: 0.0, y: 3.0 },
                        },
                        ..Default::default()
                    })
                }),
                has_header.then(|| rule::horizontal(2)),
                stack![
                    row![
                        scaffold
                            .rail
                            .map(|rail| container(rail).height(Length::Fill)),
                        has_rail.then(|| rule::vertical(2)),
                        scaffold.sidebar.map(|sidebar| {
                            container(sidebar)
                                .style(|_| container::Style {
                                    shadow: Shadow {
                                        blur_radius: 14.0,
                                        color: Color::BLACK.scale_alpha(0.45),
                                        offset: iced::Vector { x: 3.0, y: 0.0 },
                                    },
                                    ..Default::default()
                                })
                                .width(300)
                                .height(Length::Fill)
                        }),
                        has_sidebar.then(|| rule::vertical(2)),
                        scaffold.body,
                        has_context.then(|| rule::vertical(2)),
                        scaffold.context.map(|context| {
                            container(context)
                                .style(|_| container::Style {
                                    shadow: Shadow {
                                        blur_radius: 14.0,
                                        color: Color::BLACK.scale_alpha(0.45),
                                        offset: iced::Vector { x: -3.0, y: 0.0 },
                                    },
                                    ..Default::default()
                                })
                                .width(400)
                                .height(Length::Fill)
                        })
                    ]
                    .boxed()
                ]
                // Stack clips upper drawing layers; keep the row as its layout base.
                .push_under(space().boxed())
                .clip(true),
                scaffold.footer
            ],
            scaffold.dialog
        ]
    }
}
