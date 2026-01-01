use std::{panic, process, time::Duration};

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
    let _ = std::fs::remove_dir_all("./client.sled");
    let _ = std::fs::remove_dir_all("./test_data");

    let addr = "0.0.0.0:1234".to_socket_addrs().unwrap().next().unwrap();
    let (send_server, recv_server) = oneshot::channel();

    tokio::spawn(async move {
        let server = Server::build()
            .addr(addr)
            .cancel(CancellationToken::new())
            .start_server()
            .await
            .unwrap();

        debug!("server started");

        send_server.send(server).unwrap();
    });

    let host = "localhost:1234".to_owned();

    let first_connect = Client::first_connect(host.clone()).await.unwrap();

    let totp_secret = TOTP::default();
    let username = "admin".to_string();
    let password = "admin".to_string();

    match first_connect {
        crate::client::FirstConnect::Login(_) => unreachable!(),
        crate::client::FirstConnect::Init(init) => {
            init.init(
                username.clone(),
                password.clone().into_bytes(),
                totp_secret.clone(),
            )
            .await
            .unwrap();
        }
    };

    // delete to test login
    Client::remove_profile("admin@localhost:1234".into())
        .await
        .unwrap();

    // ===== TEST WRONG PASSWORD =====

    let second_connect = Client::first_connect(host.clone()).await.unwrap();

    match second_connect {
        crate::client::FirstConnect::Init(_) => unreachable!(),
        crate::client::FirstConnect::Login(login) => {
            login
                .login(
                    username.clone(),
                    b"wrong password".to_vec(),
                    totp_secret.generate_current().unwrap(),
                )
                .await
                .unwrap_err();
        }
    };

    // ===== TEST WRONG USERNAME =====

    let third_connect = Client::first_connect(host.clone()).await.unwrap();

    match third_connect {
        crate::client::FirstConnect::Init(_) => unreachable!(),
        crate::client::FirstConnect::Login(login) => {
            login
                .login(
                    "wrong username".to_string(),
                    password.clone().into_bytes(),
                    totp_secret.generate_current().unwrap(),
                )
                .await
                .unwrap_err();
        }
    };

    // ===== TEST LOGIN =====

    let fourth_connect = Client::first_connect(host.clone()).await.unwrap();

    match fourth_connect {
        crate::client::FirstConnect::Init(_) => unreachable!(),
        crate::client::FirstConnect::Login(login) => {
            login
                .login(
                    username.clone(),
                    password.clone().into_bytes(),
                    totp_secret.generate_current().unwrap(),
                )
                .await
                .unwrap();
        }
    };

    // ===== TEST CLIENT =====

    let client = Client::open_profile("admin@localhost:1234".into(), "admin".as_bytes().to_owned())
        .await
        .unwrap();

    debug!("Login successful!");

    let duration = client.ping_upstream().await.unwrap();
    debug!("ping duration: {:?}", duration);

    // ===== TEST AGENT =====

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
        debug!("agent waiting for confirmation");
        let agent = confirm.wait_for_confirm().await.unwrap();
        debug!("agent init complete!");
        debug!("starting up agent");
        agent.run().await.unwrap();
        debug!("agent has unexpectedly exited");
    });

    let client_confirm = client.add_agent_with_code(join_code).await.unwrap();

    debug!("waiting to receive confirm code");

    client_confirm
        .confirm(confirm_recv.await.unwrap())
        .await
        .unwrap();

    let mut device_list = client.watch_device_list();

    // The first change is caused by the agent being added to the device list
    device_list.changed().await.unwrap();
    // The second change is caused by the agent connecting and therefore switching
    // to online
    device_list.changed().await.unwrap();

    let device = device_list.borrow().first_key_value().unwrap().1.clone();

    if !device.item().online_status {
        panic!("Device is not online");
    }

    let ping = device.ping().await.unwrap();

    debug!("ping through forward connection: {}µs", ping.as_micros());

    client.close(Duration::from_secs(1)).await.unwrap();

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
