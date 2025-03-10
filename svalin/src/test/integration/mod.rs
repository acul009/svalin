use std::{panic, process, time::Duration};

use anyhow::Context;
use std::net::ToSocketAddrs;
use test_log::test;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use totp_rs::TOTP;
use tracing::debug;

use crate::{agent::Agent, client::Client, server::Server};

#[test(tokio::test(flavor = "multi_thread"))]
async fn integration_tests() {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        hook(panic_info);
        process::exit(1);
    }));

    // delete test dbs
    let _ = std::fs::remove_file("./client.jammdb");
    let _ = std::fs::remove_file("./server_test.jammdb");
    let _ = std::fs::remove_file("./agent.jammdb");

    let addr = "0.0.0.0:1234".to_socket_addrs().unwrap().next().unwrap();
    // delete the test db
    let db = marmelade::DB::open("./server_test.jammdb").expect("failed to open client db");

    let (send_server, recv_server) = oneshot::channel();

    tokio::spawn(async move {
        let server = Server::build()
            .addr(addr)
            .scope(
                db.scope("default".into())
                    .context("Failed to prepare server")
                    .unwrap(),
            )
            .cancel(CancellationToken::new())
            .start_server()
            .await
            .unwrap();

        debug!("server started");

        send_server.send(server).unwrap();
    });

    let host = "localhost:1234".to_owned();

    let first_connect = Client::first_connect(host.clone()).await.unwrap();

    match first_connect {
        crate::client::FirstConnect::Login(_) => unreachable!(),
        crate::client::FirstConnect::Init(init) => {
            let totp_secret = TOTP::default();
            init.init("admin".into(), "admin".into(), totp_secret)
                .await
                .unwrap();
        }
    };

    let client = Client::open_profile("admin@localhost:1234".into(), "admin".as_bytes().to_owned())
        .await
        .unwrap();

    let duration = client.ping_upstream().await.unwrap();
    debug!("ping duration: {:?}", duration);

    // test_agent
    debug!("initializing agent!");
    let waiting = Agent::init(host.clone()).await.unwrap();
    let join_code = waiting.join_code().to_owned();
    debug!("received join code");
    let (confirm_send, confirm_recv) = oneshot::channel();

    let agent_handle = tokio::spawn(async move {
        let confirm = waiting.wait_for_init().await.unwrap();
        debug!("generated confirm code");
        confirm_send
            .send(confirm.confirm_code().to_owned())
            .unwrap();
        let agent = confirm.wait_for_confirm().await.unwrap();
        debug!("agent init complete!");
        debug!("starting up agent");
        agent.run().await.unwrap();
    });

    let client_confirm = client.add_agent_with_code(join_code).await.unwrap();

    debug!("waiting for use to confirm agent join");

    client_confirm
        .confirm(confirm_recv.await.unwrap(), "test agent".into())
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(5)).await;

    let device = client.device_list().first_key_value().unwrap().1.clone();

    let ping = device.ping().await.unwrap();

    debug!("ping through forward connection: {}µs", ping.as_micros());

    client.close();

    debug!("closing server");

    // TODO: make this actually work properly
    let _ = recv_server
        .await
        .unwrap()
        .close(Duration::from_secs(1))
        .await
        .unwrap();

    debug!("server closed");

    agent_handle.abort();

    // TODO: actually program this so you can shutdown the programm in a controlled
    // manner again
    process::exit(0);
}
