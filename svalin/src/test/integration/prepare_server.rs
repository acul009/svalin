use std::net::ToSocketAddrs;

use anyhow::{Context, Result};

use crate::server::Server;

pub async fn prepare_server() -> Result<Server> {
    let addr = "0.0.0.0:1234".to_socket_addrs().unwrap().next().unwrap();
    // delete the test db
    std::fs::remove_file("./server_test.jammdb").unwrap_or(());
    let db = marmelade::DB::open("./server_test.jammdb").expect("failed to open client db");
    let server = Server::prepare(
        addr,
        db.scope("default".into())
            .context("Failed to prepare server")
            .unwrap(),
    )
    .await
    .unwrap();

    Ok(server)
}
