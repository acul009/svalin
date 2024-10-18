use std::net::ToSocketAddrs;

use test_log::test;
use tls_test_command::{TlsTest, TlsTestCommandHandler};
use tracing::debug;

mod tls_test_command;

use crate::{
    commands::ping::{Ping, PingHandler},
    permissions::{
        anonymous_permission_handler::AnonymousPermissionHandler,
        whitelist::WhitelistPermissionHandler, DummyPermission,
    },
    rpc::{
        client::RpcClient, command::handler::HandlerCollection, connection::Connection,
        server::RpcServer,
    },
    verifiers::skip_verify::{SkipClientVerification, SkipServerVerification},
};

#[test(tokio::test)]
async fn ping_test() {
    println!("starting ping test");

    let address = "127.0.0.1:1234";
    let credentials = svalin_pki::Keypair::generate()
        .unwrap()
        .to_self_signed_cert()
        .unwrap();
    let server = RpcServer::new(
        address.to_socket_addrs().unwrap().next().unwrap(),
        &credentials,
        SkipClientVerification::new(),
    )
    .unwrap();

    let permission_handler = AnonymousPermissionHandler;

    let commands = HandlerCollection::<_, DummyPermission>::new(permission_handler);
    commands.chain().await.add(PingHandler::new());

    let server_handle = tokio::spawn(async move {
        server.run(commands).await.unwrap();
    });

    debug!("trying to connect client");

    let client = RpcClient::connect(address, None, SkipServerVerification::new())
        .await
        .unwrap();

    debug!("client connected");

    let connection = client.upstream_connection();

    let _ping = connection.dispatch(Ping).await.unwrap();

    server_handle.abort();
}

#[test(tokio::test)]
async fn tls_test() {
    println!("starting tls test");

    let address = "127.0.0.1:1235";
    let credentials = svalin_pki::Keypair::generate()
        .unwrap()
        .to_self_signed_cert()
        .unwrap();
    let server = RpcServer::new(
        address.to_socket_addrs().unwrap().next().unwrap(),
        &credentials,
        SkipClientVerification::new(),
    )
    .unwrap();

    let permission_handler = AnonymousPermissionHandler;

    let commands = HandlerCollection::<_, DummyPermission>::new(permission_handler);
    commands
        .chain()
        .await
        .add(TlsTestCommandHandler::new().unwrap());

    let server_handle = tokio::spawn(async move {
        server.run(commands).await.unwrap();
    });

    debug!("trying to connect client");

    let client = RpcClient::connect(address, None, SkipServerVerification::new())
        .await
        .unwrap();

    debug!("client connected");

    let connection = client.upstream_connection();

    connection.dispatch(TlsTest).await.unwrap();

    server_handle.abort();
}

#[test(tokio::test)]
async fn perm_test() {
    println!("starting permission test");

    let address = "127.0.0.1:1236";
    let credentials = svalin_pki::Keypair::generate()
        .unwrap()
        .to_self_signed_cert()
        .unwrap();
    let server = RpcServer::new(
        address.to_socket_addrs().unwrap().next().unwrap(),
        &credentials,
        SkipClientVerification::new(),
    )
    .unwrap();

    let permission_handler = WhitelistPermissionHandler::new(vec![]);

    let commands = HandlerCollection::<_, DummyPermission>::new(permission_handler);
    commands
        .chain()
        .await
        .add(TlsTestCommandHandler::new().unwrap());

    let server_handle = tokio::spawn(async move {
        server.run(commands).await.unwrap();
    });

    debug!("trying to connect client");

    let client = RpcClient::connect(address, None, SkipServerVerification::new())
        .await
        .unwrap();

    debug!("client connected");

    let connection = client.upstream_connection();

    connection.dispatch(TlsTest).await.unwrap_err();

    server_handle.abort();
}
