use std::net::ToSocketAddrs;

use test_log::test;
use tracing::debug;

use crate::{
    ping::{pingDispatcher, PingHandler},
    skip_verify::{SkipClientVerification, SkipServerVerification},
    Client, HandlerCollection,
};

#[test(tokio::test(flavor = "multi_thread"))]
async fn ping_test() {
    println!("starting ping test");

    let address = "127.0.0.1:1234";
    let credentials = svalin_pki::Keypair::generate()
        .unwrap()
        .to_self_signed_cert()
        .unwrap();
    let mut server = crate::Server::new(
        address.to_socket_addrs().unwrap().next().unwrap(),
        &credentials,
        SkipClientVerification::new(),
    )
    .unwrap();
    let commands = HandlerCollection::new();
    commands.add(PingHandler::new()).await;

    let server_handle = tokio::spawn(async move {
        server.run(commands).await.unwrap();
    });

    debug!("trying to connect client");

    let client = Client::connect(address, None, SkipServerVerification::new())
        .await
        .unwrap();

    debug!("client connected");

    let mut connection = client.upstream_connection();

    let ping = connection.ping().await.unwrap();

    server_handle.abort();
}
