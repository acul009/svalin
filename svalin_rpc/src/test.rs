use std::{net::ToSocketAddrs, time::Duration};

use tokio::time::sleep;

use crate::HandlerCollection;

#[tokio::test]
async fn ping_test() {
    let address = "127.0.0.1:1234".to_socket_addrs().unwrap().next().unwrap();
    let credentials = svalin_pki::Keypair::generate()
        .unwrap()
        .to_self_signed_cert()
        .unwrap();
    let mut server = crate::Server::new(address, &credentials).unwrap();
    let commands = HandlerCollection::new();

    let server_handle = tokio::spawn(async move {
        server.run(commands).await.unwrap();
    });

    sleep(Duration::from_millis(3000)).await;

    server_handle.abort();
}
