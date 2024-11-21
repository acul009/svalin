use svalin_rpc::rpc::connection::Connection;
use thiserror::Error;

pub struct TcpTunnelConfig {
    local_port: u16,
    remote_host: String,
}

pub struct TcpTunnel<Connection> {
    config: TcpTunnelConfig,
    connection: Connection,
}

#[derive(Debug, Error)]
pub enum TcpTunnelCreateError {}

#[derive(Debug, Error)]
pub enum TcpTunnelRunError {}

#[derive(Debug, Error)]
pub enum TcpTunnelCloseError {}

impl<C> TcpTunnel<C>
where
    C: Connection,
{
    pub fn open(connection: C, config: TcpTunnelConfig) -> Result<Self, TcpTunnelCreateError> {
        todo!()
    }

    pub async fn run(&self) -> Result<(), TcpTunnelRunError> {
        todo!()
    }

    pub fn close(&self) -> Result<(), TcpTunnelCloseError> {
        todo!()
    }
}
