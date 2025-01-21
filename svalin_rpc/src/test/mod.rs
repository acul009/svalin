use std::{net::ToSocketAddrs, panic, process, time::Duration};

use test_log::test;
use tls_test_command::{TlsTest, TlsTestCommandHandler};
use tokio_util::sync::CancellationToken;
use tracing::debug;

mod tls_test_command;

use crate::{
    commands::ping::{Ping, PingHandler},
    permissions::{
        anonymous_permission_handler::AnonymousPermissionHandler,
        whitelist::WhitelistPermissionHandler, DummyPermission,
    },
    rpc::{
        client::RpcClient,
        command::handler::HandlerCollection,
        connection::Connection,
        server::{build_rpc_server, create_rpc_socket},
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

    let permission_handler = AnonymousPermissionHandler::<DummyPermission>::default();

    let commands = HandlerCollection::new(permission_handler);
    commands.chain().await.add(PingHandler);

    let socket = create_rpc_socket(address.to_socket_addrs().unwrap().next().unwrap()).unwrap();

    let server = build_rpc_server()
        .credentials(credentials)
        .commands(commands)
        .client_cert_verifier(SkipClientVerification::new())
        .cancellation_token(CancellationToken::new())
        .start_server(socket)
        .unwrap();

    debug!("trying to connect client");

    let client = RpcClient::connect(address, None, SkipServerVerification::new())
        .await
        .unwrap();

    debug!("client connected");

    let connection = client.upstream_connection();

    let _ping = connection.dispatch(Ping).await.unwrap();

    server.close(Duration::from_secs(1)).await.unwrap();
}

#[test(tokio::test)]
async fn tls_test() {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        hook(panic_info);
        process::exit(1);
    }));

    println!("starting tls test");

    let address = "127.0.0.1:1235";
    let credentials = svalin_pki::Keypair::generate()
        .unwrap()
        .to_self_signed_cert()
        .unwrap();
    let socket = create_rpc_socket(address.to_socket_addrs().unwrap().next().unwrap()).unwrap();

    let permission_handler = AnonymousPermissionHandler::<DummyPermission>::default();

    let commands = HandlerCollection::new(permission_handler);
    commands
        .chain()
        .await
        .add(TlsTestCommandHandler::new().unwrap());

    let server = build_rpc_server()
        .credentials(credentials)
        .commands(commands)
        .client_cert_verifier(SkipClientVerification::new())
        .cancellation_token(CancellationToken::new())
        .start_server(socket)
        .unwrap();

    debug!("trying to connect client");

    let client = RpcClient::connect(address, None, SkipServerVerification::new())
        .await
        .unwrap();

    debug!("client connected");

    let connection = client.upstream_connection();

    connection.dispatch(TlsTest).await.unwrap();

    server.close(Duration::from_secs(1)).await.unwrap();
}

#[test(tokio::test(flavor = "multi_thread"))]
async fn perm_test() {
    println!("starting permission test");

    let address = "127.0.0.1:1236";
    let credentials = svalin_pki::Keypair::generate()
        .unwrap()
        .to_self_signed_cert()
        .unwrap();
    let socket = create_rpc_socket(address.to_socket_addrs().unwrap().next().unwrap()).unwrap();

    let permission_handler = WhitelistPermissionHandler::<DummyPermission>::new(vec![]);

    let commands = HandlerCollection::new(permission_handler);
    commands
        .chain()
        .await
        .add(TlsTestCommandHandler::new().unwrap());

    let server = build_rpc_server()
        .credentials(credentials)
        .commands(commands)
        .client_cert_verifier(SkipClientVerification::new())
        .cancellation_token(CancellationToken::new())
        .start_server(socket)
        .unwrap();

    debug!("trying to connect client");

    let client = RpcClient::connect(address, None, SkipServerVerification::new())
        .await
        .unwrap();

    debug!("client connected");

    let connection = client.upstream_connection();

    connection.dispatch(TlsTest).await.unwrap_err();

    server.close(Duration::from_secs(1)).await.unwrap();
}
