use std::{process, time::Duration};

use clap::error;
use prepare_server::prepare_server;
use svalin_rpc::commands::ping::pingDispatcher;
use test_log::test;
use tokio::sync::oneshot;
use totp_rs::TOTP;
use tracing::debug;

use crate::{agent::Agent, client::Client, server::Server};

mod prepare_server;
// mod test_init;

#[test(tokio::test(flavor = "multi_thread"))]
async fn integration_tests() {
    let server_handle = tokio::spawn(async move {
        let mut server = prepare_server().await.unwrap();
        server.run().await.unwrap();
    });

    // delete test client db
    std::fs::remove_file("./client.jammdb").unwrap_or(());

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

    let conn = client.rpc().upstream_connection();

    let duration = conn.ping().await.unwrap();
    debug!("ping duration: {:?}", duration);

    // test_agent
    let waiting = Agent::init(host.clone()).await.unwrap();
    let join_code = waiting.join_code().to_owned();
    let (confirm_send, confirm_recv) = oneshot::channel();

    let agent_handle = tokio::spawn(async move {
        let confirm = waiting.wait_for_init().await.unwrap();
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

    tokio::time::sleep(Duration::from_secs(10)).await;

    client.close();

    server_handle.abort();

    agent_handle.await.unwrap();

    tracing::error!("TODO: make ping work by implementing forwarding, e2e encryption and basic agent session serving");

    process::exit(1);
}
