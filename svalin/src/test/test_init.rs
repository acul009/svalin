use std::net::ToSocketAddrs;

use svalin_rpc::ping::pingDispatcher;
use test_log::test;
use totp_rs::TOTP;
use tracing::debug;

use crate::{client::Client, server::Server};

#[test(tokio::test(flavor = "multi_thread"))]
async fn test_init() {
    let (send_init, recv_init) = tokio::sync::oneshot::channel::<()>();

    let server_handle = tokio::spawn(async {
        let addr = "0.0.0.0:1234".to_socket_addrs().unwrap().next().unwrap();
        // delete the test db
        std::fs::remove_file("./server_test.jammdb").unwrap_or(());
        let db = marmelade::DB::open("./server_test.jammdb").expect("failed to open client db");
        let mut server = Server::prepare(addr, db.scope("default".into()).unwrap())
            .await
            .unwrap();

        send_init.send(()).unwrap();

        server.run().await.unwrap();
    });

    // delete test client db
    std::fs::remove_file("./client.jammdb").unwrap_or(());

    let host = "localhost:1234".to_owned();

    match Client::first_connect(host).await.unwrap() {
        crate::client::FirstConnect::Init(init) => {
            let totp_secret = TOTP::default();
            init.init("admin".into(), "admin".into(), totp_secret)
                .await
                .unwrap();
        }
        crate::client::FirstConnect::Login(_) => {
            panic!("Server should have been uninitialized")
        }
    };

    recv_init.await.unwrap();

    let client = Client::open_profile("admin@localhost:1234".into(), "admin".as_bytes().to_owned())
        .await
        .unwrap();

    let mut conn = client.rpc().upstream_connection();

    let duration = conn.ping().await.unwrap();
    debug!("ping duration: {:?}", duration);

    client.close();

    server_handle.abort();
    let _ = server_handle.await;
}
