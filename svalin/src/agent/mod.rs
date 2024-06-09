use anyhow::Result;
use svalin_rpc::rpc::client::RpcClient;
use svalin_rpc::skip_verify::SkipServerVerification;
use tracing::debug;

mod init;

use crate::shared::commands::public_server_status::get_public_statusDispatcher;

pub struct Agent {
    rpc: RpcClient,
}

impl Agent {}
