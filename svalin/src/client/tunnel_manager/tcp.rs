use std::collections::HashMap;

use svalin_rpc::rpc::command::dispatcher::CommandDispatcher;
use tokio::net::TcpListener;

pub struct TcpTunnelDispatcher {
    listener: TcpListener,
}

impl CommandDispatcher for TcpTunnelDispatcher {
    type Output = ();

    type Error = anyhow::Error;

    type Request = ();

    fn key() -> String {
        "forward-tcp".into()
    }

    fn get_request(&self) -> &Self::Request {
        &()
    }

    async fn dispatch(
        self,
        session: &mut svalin_rpc::rpc::session::Session,
    ) -> Result<Self::Output, Self::Error> {
        let connections = HashMap::new();

        loop {
            let (conn, addr) = self.listener.accept().await?;
            // drop connections not from localhost
            if !addr.ip().is_loopback() {}
        }
    }
}
