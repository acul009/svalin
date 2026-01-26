use anyhow::Result;
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    session::{Session, SessionReadError},
};
use tokio::{io::copy_bidirectional, net::TcpStream, select, sync::watch};

use super::handler::{TcpForwardError, TcpForwardHandler};

pub struct TcpForwardDispatcher {
    pub target: String,
    pub stream: TcpStream,
    pub active: watch::Receiver<bool>,
}

#[derive(Debug, thiserror::Error)]
pub enum TcpForwardDispatcherError {
    #[error("error reading answer from relaying party: {0}")]
    ReadAnswerError(SessionReadError),
    #[error("error from relaying party: {0}")]
    ForwardError(TcpForwardError),
    #[error("error copying data: {0}")]
    CopyError(std::io::Error),
}

impl CommandDispatcher for TcpForwardDispatcher {
    type Output = ();
    type Error = TcpForwardDispatcherError;

    type Request = String;

    fn key() -> String {
        TcpForwardHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &self.target
    }

    async fn dispatch(mut self, session: &mut Session) -> Result<Self::Output, Self::Error> {
        session
            .read_object::<Result<(), TcpForwardError>>()
            .await
            .map_err(TcpForwardDispatcherError::ReadAnswerError)?
            .map_err(TcpForwardDispatcherError::ForwardError)?;

        let transport = session.borrow_transport();

        let copy_future = copy_bidirectional(transport, &mut self.stream);

        select! {
            copy_result = copy_future => {copy_result.map_err(TcpForwardDispatcherError::CopyError)?; return Ok(())},
            _ = self.active.changed() => {
                if !*self.active.borrow() {
                    return Ok(());
                }

                // No idea how to solve this, since a loop means copy_result is moved
                todo!()
            },
        }
    }
}
