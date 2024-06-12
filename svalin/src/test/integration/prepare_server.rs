use std::net::ToSocketAddrs;

use anyhow::Result;

use crate::server::Server;

pub async fn prepare_server() -> Result<Server> {
    let addr = "0.0.0.0:1234".to_socket_addrs().unwrap().next().unwrap();
    // delete the test db
    std::fs::remove_file("./server_test.jammdb").unwrap_or(());
    let db = marmelade::DB::open("./server_test.jammdb").expect("failed to open client db");
    let mut server = Server::prepare(addr, db.scope("default".into()).unwrap())
        .await
        .unwrap();

    Ok(server)
}
