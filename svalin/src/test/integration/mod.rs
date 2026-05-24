use std::{panic, process, time::Duration};

use std::net::ToSocketAddrs;
use svalin_client_store::persistent::{self, SvalinMetaInfo};
use svalin_pki::get_current_timestamp;
use test_log::test;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use totp_rs::TOTP;

use crate::client::state::ClientStateUpdate;
use crate::{agent::Agent, client::Client, server::Server};

#[test(tokio::test(flavor = "multi_thread"))]
async fn integration_tests() {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        hook(panic_info);
        process::exit(1);
    }));

    let port = rand::random_range(1025..65000);
    let host = format!("localhost:{port}");

    // delete test dbs
    let _ = std::fs::remove_dir_all("./test_data");

    let addr = format!("0.0.0.0:{port}")
        .as_str()
        .to_socket_addrs()
        .unwrap()
        .next()
        .unwrap();
    let server_cancel = CancellationToken::new();
    let cancel = server_cancel.clone();
    let (send_server, recv_server) = oneshot::channel();

    tokio::spawn(async move {
        let server = Server::build()
            .addr(addr)
            .cancel(cancel)
            .start_server()
            .await
            .unwrap();

        tracing::trace!("server started");

        send_server.send(server).unwrap();
    });

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

    let profile_name = format!("admin@{host}");

    // delete to test login
    Client::remove_profile(&profile_name).await.unwrap();

    // // ===== TEST WRONG PASSWORD =====

    // let second_connect = Client::first_connect(host.clone()).await.unwrap();

    // match second_connect {
    //     crate::client::FirstConnect::Init(_) => unreachable!(),
    //     crate::client::FirstConnect::Login(login) => {
    //         login
    //             .login(
    //                 username.clone(),
    //                 b"wrong password".to_vec(),
    //                 totp_secret.generate_current().unwrap(),
    //             )
    //             .await
    //             .unwrap_err();
    //     }
    // };

    // // ===== TEST WRONG USERNAME =====

    // let third_connect = Client::first_connect(host.clone()).await.unwrap();

    // match third_connect {
    //     crate::client::FirstConnect::Init(_) => unreachable!(),
    //     crate::client::FirstConnect::Login(login) => {
    //         login
    //             .login(
    //                 "wrong username".to_string(),
    //                 password.clone().into_bytes(),
    //                 totp_secret.generate_current().unwrap(),
    //             )
    //             .await
    //             .unwrap_err();
    //     }
    // };

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

    let client_cancel = CancellationToken::new();

    let client = Client::open_profile(
        &profile_name,
        "admin".as_bytes().to_owned(),
        client_cancel.clone(),
    )
    .await
    .unwrap();

    let (mut client_state, mut client_state_updates) = client.subscribe_state().await.unwrap();

    tracing::trace!("Login successful!");

    let duration = client.ping_upstream().await.unwrap();
    tracing::trace!("ping duration: {:?}", duration);

    // // wait for the first full update - this is sent after generating the key packages
    // let update = timeout(Duration::from_secs(30), client_state_updates.recv())
    //     .await
    //     .unwrap()
    //     .unwrap();
    // if let ClientStateUpdate::Persistent(persistent::Message::UpdateFromMainState(_)) = &update {
    //     client_state.update(update);
    // } else {
    //     panic!("expected persistent state update, got: {:?}", &update);
    // }
    // wait a short while to ensure the client has generated some key packages
    tokio::time::sleep(Duration::from_secs(1)).await;

    // ===== TEST AGENT =====

    tracing::trace!("initializing agent!");
    let waiting = Agent::init(host.clone()).await.unwrap();
    let join_code = waiting.join_code().to_owned();
    tracing::trace!("received join code");
    let (confirm_send, confirm_recv) = oneshot::channel();

    let agent_cancel = CancellationToken::new();
    let cancel = agent_cancel.clone();
    let agent_handle = tokio::spawn(async move {
        let confirm = waiting.wait_for_init().await.unwrap();
        tracing::trace!("generated confirm code");
        confirm_send
            .send(confirm.confirm_code().to_owned())
            .unwrap();
        tracing::trace!("agent waiting for confirmation");
        confirm.wait_for_confirm(cancel.clone()).await.unwrap();
        Agent::run(cancel).await.unwrap()
    });

    let (send, recv) = oneshot::channel();
    let client2 = client.clone();
    let add_agent_handle =
        tokio::spawn(async move { client2.add_agent_with_code(join_code, send).await });

    tracing::trace!("waiting to receive confirm code");

    let confirm = recv.await.unwrap();
    confirm.send(confirm_recv.await.unwrap()).unwrap();

    add_agent_handle.await.unwrap().unwrap();
    tracing::trace!("agent was added");

    // first update should be the online status
    let update = timeout(Duration::from_secs(5), client_state_updates.recv())
        .await
        .unwrap()
        .unwrap();
    if let ClientStateUpdate::AgentOnlineStatus(_, true) = &update {
        client_state.update(update);
        tracing::trace!("agent is online");
    } else {
        panic!("expected agent online status update, got: {:?}", &update);
    }

    // second update should be the system report
    let update = timeout(Duration::from_secs(5), client_state_updates.recv())
        .await
        .unwrap()
        .unwrap();
    if let ClientStateUpdate::Persistent(persistent::Message::UpdateFromMainState(_)) = &update {
        client_state.update(update);
    } else {
        panic!("expected update from main status update, got {:?}", &update);
    }

    let system_report = client_state.persistent().iter().next().unwrap();
    tracing::trace!("client persistent data: {:#?}", system_report);

    // testing device handle
    let agent_spki_hash = client_state.persistent().iter().next().unwrap().0.clone();
    let device = client.device(agent_spki_hash.clone());
    device.ping().await.unwrap();

    // testing directly receiving system report
    device.request_system_report().await.unwrap();

    let update = timeout(Duration::from_secs(1), client_state_updates.recv())
        .await
        .unwrap()
        .unwrap();
    if let ClientStateUpdate::Persistent(persistent::Message::UpdateSystemReport(_, _)) = &update {
        client_state.update(update);
    } else {
        panic!("expected agent online status update, got: {:?}", &update);
    }

    // testing sending meta info
    tokio::time::sleep(Duration::from_secs(1)).await;

    let meta_info = SvalinMetaInfo {
        updated_at: get_current_timestamp(),
        name: "Test Device".into(),
        group: "Test Group".into(),
    };
    device.update_metainfo(meta_info).await.unwrap();

    let update = timeout(Duration::from_secs(5), client_state_updates.recv())
        .await
        .unwrap()
        .unwrap();
    if let ClientStateUpdate::Persistent(persistent::Message::UpdateMetaInfo(_, _)) = &update {
        client_state.update(update);
    } else {
        panic!("expected update from main status update, got {:?}", &update);
    }

    assert_eq!(
        client_state
            .persistent()
            .get(&agent_spki_hash)
            .unwrap()
            .meta_info()
            .unwrap()
            .name,
        "Test Device"
    );

    // =====================================================================
    // Controlled shutdown
    // =====================================================================

    // controlled agent shutdown
    agent_cancel.cancel();
    tokio::time::timeout(Duration::from_secs(1), agent_handle)
        .await
        .unwrap()
        .unwrap();

    // agent should go offline
    let update = timeout(Duration::from_secs(1), client_state_updates.recv())
        .await
        .unwrap()
        .unwrap();
    if let ClientStateUpdate::AgentOnlineStatus(_, false) = &update {
        client_state.update(update);
        tracing::trace!("agent is offline");
    } else {
        panic!("expected agent online status update, got: {:?}", &update);
    }

    // controlled client shutdown
    client.close(Duration::from_secs(3)).await.unwrap();

    // controlled server shutdown
    server_cancel.cancel();
    tokio::time::timeout(Duration::from_secs(1), recv_server)
        .await
        .unwrap()
        .unwrap();

    process::exit(0);
}
