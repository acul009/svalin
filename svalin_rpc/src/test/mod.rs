use std::net::ToSocketAddrs;

use test_log::test;
use tls_test_command::{tls_testDispatcher, TlsTestCommandHandler};
use tracing::debug;
use url::Url;

mod tls_test_command;

use crate::{
    commands::ping::{pingDispatcher, PingHandler},
    rpc::{client::RpcClient, command::HandlerCollection, server::RpcServer},
    verifiers::skip_verify::{SkipClientVerification, SkipServerVerification},
};

#[test(tokio::test(flavor = "multi_thread"))]
async fn ping_test() {
    println!("starting ping test");

    let url = Url::parse("127.0.0.1:1234").unwrap();
    let credentials = svalin_pki::Keypair::generate()
        .unwrap()
        .to_self_signed_cert()
        .unwrap();
    let server = RpcServer::new(
        (url.host_str().unwrap(), url.port().unwrap())
            .to_socket_addrs()
            .unwrap()
            .next()
            .unwrap(),
        &credentials,
        SkipClientVerification::new(),
    )
    .unwrap();
    let commands = HandlerCollection::new();
    commands.chain().await.add(PingHandler::new());

    let server_handle = tokio::spawn(async move {
        server.run(commands).await.unwrap();
    });

    debug!("trying to connect client");

    let client = RpcClient::connect(&url, None, SkipServerVerification::new())
        .await
        .unwrap();

    debug!("client connected");

    let connection = client.upstream_connection();

    let _ping = connection.ping().await.unwrap();

    server_handle.abort();
}

#[test(tokio::test(flavor = "multi_thread"))]
async fn tls_test() {
    println!("starting tls test");

    let url = Url::parse("127.0.0.1:1235").unwrap();
    let credentials = svalin_pki::Keypair::generate()
        .unwrap()
        .to_self_signed_cert()
        .unwrap();
    let server = RpcServer::new(
        (url.host_str().unwrap(), url.port().unwrap())
            .to_socket_addrs()
            .unwrap()
            .next()
            .unwrap(),
        &credentials,
        SkipClientVerification::new(),
    )
    .unwrap();
    let commands = HandlerCollection::new();
    commands
        .chain()
        .await
        .add(TlsTestCommandHandler::new().unwrap());

    let server_handle = tokio::spawn(async move {
        server.run(commands).await.unwrap();
    });

    debug!("trying to connect client");

    let client = RpcClient::connect(&url, None, SkipServerVerification::new())
        .await
        .unwrap();

    debug!("client connected");

    let mut connection = client.upstream_connection();

    connection.tls_test().await.unwrap();

    server_handle.abort();
}
