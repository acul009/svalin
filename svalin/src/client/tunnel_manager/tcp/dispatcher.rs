use anyhow::{anyhow, Result};
use async_trait::async_trait;
use svalin_rpc::{
    rpc::{
        command::{
            dispatcher::{CommandDispatcher, TakeableCommandDispatcher},
            handler::CommandHandler,
        },
        session::Session,
    },
    transport::combined_transport::CombinedTransport,
};
use tokio::{io::copy_bidirectional, net::TcpStream, select, sync::watch};

use super::handler::{TcpForwardError, TcpForwardHandler};

pub struct TcpForwardDispatcher {
    pub target: String,
    pub stream: TcpStream,
    pub active: watch::Receiver<bool>,
}

#[async_trait]
impl TakeableCommandDispatcher for TcpForwardDispatcher {
    type Output = ();

    type Request = String;

    fn key() -> String {
        TcpForwardHandler::key()
    }

    fn get_request(&self) -> Self::Request {
        self.target.clone()
    }

    async fn dispatch(
        mut self,
        session: &mut Option<Session>,
        _request: Self::Request,
    ) -> Result<Self::Output> {
        if let Some(mut session) = session.take() {
            let _forward_active = session.read_object::<Result<(), TcpForwardError>>().await?;

            let (transport_read, transport_write) = session.borrow_transport();
            let mut transport = CombinedTransport::new(transport_read, transport_write);

            let copy_future = copy_bidirectional(&mut transport, &mut self.stream);

            select! {
                copy_result = copy_future => {copy_result?; return Ok(())},
                _ = self.active.changed() => {
                    if !*self.active.borrow() {
                        return Ok(());
                    }

                    // No idea how to solve this, since a loop means copy_result is moved
                    todo!()
                },
            }
        } else {
            Err(anyhow!("tried dispatching command with None"))
        }
    }
}
