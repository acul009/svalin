use std::{collections::HashMap, time::Duration};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_rpc::rpc::{
    command::{
        dispatcher::CommandDispatcher,
        handler::{CommandHandler, PermissionPrecursor},
    },
    session::Session,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    select,
    sync::{mpsc, oneshot},
    time::Instant,
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use crate::permissions::Permission;

const TCP_TUNNEL_KEY: &str = "forward-tcp";
const TCP_TUNNEL_IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub struct TcpTunnelDispatcher {
    pub listener: TcpListener,
    pub cancel: CancellationToken,
    pub ready: oneshot::Sender<u16>,
    pub remote_host: String,
}

#[derive(Serialize, Deserialize)]
enum ToAgent {
    Opened(u64),
    Closed(u64),
    Data(u64, Vec<u8>),
}

#[derive(Serialize, Deserialize)]
enum ToClient {
    Closed(u64),
    Data(u64, Vec<u8>),
}

impl ToClient {
    fn id(&self) -> u64 {
        match self {
            ToClient::Closed(id) => *id,
            ToClient::Data(id, _) => *id,
        }
    }
}

impl ToAgent {
    fn id(&self) -> u64 {
        match self {
            ToAgent::Opened(id) | ToAgent::Closed(id) | ToAgent::Data(id, _) => *id,
        }
    }
}

impl CommandDispatcher for TcpTunnelDispatcher {
    type Output = ();

    type Error = anyhow::Error;

    type Request = String;

    fn key() -> String {
        TCP_TUNNEL_KEY.into()
    }

    fn get_request(&self) -> &Self::Request {
        &self.remote_host
    }

    async fn dispatch(
        self,
        session: &mut svalin_rpc::rpc::session::Session,
    ) -> Result<Self::Output, Self::Error> {
        let mut connections: HashMap<u64, mpsc::Sender<ToClient>> = HashMap::new();
        let mut id_counter: u64 = 0;
        let (all_send, mut all_recv) = tokio::sync::mpsc::channel(100);
        let tasks = TaskTracker::new();
        let idle_timeout = tokio::time::sleep(TCP_TUNNEL_IDLE_TIMEOUT);
        tokio::pin!(idle_timeout);

        let _ = self.ready.send(
            self.listener
                .local_addr()
                .expect("I already set the local address")
                .port(),
        );

        loop {
            select! {
                _ = self.cancel.cancelled() => {
                    break;
                }
                _ = &mut idle_timeout, if connections.is_empty() => {
                    break;
                }
                accept_result = self.listener.accept() => {
                    let (conn, addr) = accept_result?;
                    // drop connections not from localhost
                    if !addr.ip().is_loopback() {
                        continue;
                    }
                    let id = id_counter;
                    id_counter += 1;
                    let (send, recv) = mpsc::channel(100);
                    connections.insert(id, send);
                    session.write_object(&ToAgent::Opened(id)).await?;

                    tasks.spawn(copy_conn_task(id, recv, conn, all_send.clone()));
                }
                to_agent = all_recv.recv() => {
                    if let Some(to_agent) = to_agent {
                        if let ToAgent::Closed(id) = to_agent {
                            connections.remove(&id);
                            if connections.is_empty() {
                                idle_timeout.as_mut().reset(Instant::now() + TCP_TUNNEL_IDLE_TIMEOUT);
                            }
                        }
                        session.write_object(&to_agent).await?;
                    }
                }
                to_client = session.read_object::<ToClient>() => {
                    let to_client = to_client?;
                    let id = to_client.id();
                    if let Some(channel) = connections.get_mut(&id) {
                        channel.send(to_client).await?;
                    }
                }
            }
        }

        all_recv.close();
        drop(connections);
        tasks.close();
        if let Err(_err) = tokio::time::timeout(Duration::from_secs(5), tasks.wait()).await {
            tracing::error!("Failed to close all tunnel helper tasks in time");
        };

        Ok(())
    }
}

#[derive(Default)]
pub struct TcpForwardHandler;

impl From<&PermissionPrecursor<TcpForwardHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<TcpForwardHandler>) -> Self {
        Permission::RootOnlyPlaceholder
    }
}

#[async_trait]
impl CommandHandler for TcpForwardHandler {
    type Request = String;

    fn key() -> String {
        TCP_TUNNEL_KEY.into()
    }

    async fn handle(
        &self,
        session: &mut Session,
        remote_host: Self::Request,
        cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let mut connections: HashMap<u64, mpsc::Sender<ToAgent>> = HashMap::new();
        let (all_send, mut all_recv) = mpsc::channel(100);
        let tasks = TaskTracker::new();

        loop {
            select! {
                _ = cancel.cancelled() => break,
                to_agent = session.read_object::<ToAgent>() => {
                    let to_agent = to_agent?;
                    let id = to_agent.id();

                    match to_agent {
                        ToAgent::Opened(_) => {
                            let (send, recv) = mpsc::channel(100);
                            connections.insert(id, send);
                            tasks.spawn(copy_remote_conn_task(
                                id,
                                remote_host.clone(),
                                recv,
                                all_send.clone(),
                            ));
                        }
                        ToAgent::Closed(_) => {
                            connections.remove(&id);
                        }
                        packet @ ToAgent::Data(_, _) => {
                            if let Some(connection) = connections.get_mut(&id)
                                && connection.send(packet).await.is_err()
                            {
                                connections.remove(&id);
                            }
                        }
                    }
                }
                to_client = all_recv.recv() => {
                    let Some(to_client) = to_client else {
                        break;
                    };
                    if let ToClient::Closed(id) = &to_client {
                        connections.remove(id);
                    }
                    session.write_object(&to_client).await?;
                }
            }
        }

        all_recv.close();
        drop(connections);
        tasks.close();
        if tokio::time::timeout(Duration::from_secs(5), tasks.wait())
            .await
            .is_err()
        {
            tracing::error!("Failed to close all remote tunnel helper tasks in time");
        }

        Ok(())
    }
}

async fn copy_remote_conn_task(
    id: u64,
    remote_host: String,
    mut recv: mpsc::Receiver<ToAgent>,
    to_all: mpsc::Sender<ToClient>,
) {
    let mut conn = match TcpStream::connect(&remote_host).await {
        Ok(conn) => conn,
        Err(err) => {
            tracing::error!("Failed to connect tunnel stream to {remote_host}: {err}");
            let _ = to_all.send(ToClient::Closed(id)).await;
            return;
        }
    };
    let mut buffer = vec![0; 1024];

    loop {
        select! {
            read = conn.read(&mut buffer) => {
                match read {
                    Ok(0) => {
                        let _ = to_all.send(ToClient::Closed(id)).await;
                        break;
                    }
                    Ok(read) => {
                        if to_all
                            .send(ToClient::Data(id, buffer[..read].to_vec()))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(err) => {
                        tracing::error!("Error reading from remote tunnel connection: {err}");
                        let _ = to_all.send(ToClient::Closed(id)).await;
                        break;
                    }
                }
            }
            from_client = recv.recv() => {
                match from_client {
                    Some(ToAgent::Data(_, data)) => {
                        if let Err(err) = conn.write_all(&data).await {
                            tracing::error!("Error writing to remote tunnel connection: {err}");
                            let _ = to_all.send(ToClient::Closed(id)).await;
                            break;
                        }
                    }
                    Some(ToAgent::Closed(_)) | None => break,
                    Some(ToAgent::Opened(_)) => {}
                }
            }
        }
    }
}

async fn copy_conn_task(
    id: u64,
    mut recv: mpsc::Receiver<ToClient>,
    mut conn: TcpStream,
    to_all: mpsc::Sender<ToAgent>,
) {
    let mut buffer = vec![0; 1024];
    loop {
        select! {
            read =
                conn.read(&mut buffer) => {
                    match read {
                        Ok(read) =>  {
                            if read == 0 {
                                let _ = to_all.send(ToAgent::Closed(id)).await;
                                break;
                            }
                            let data = buffer[..read].to_vec();
                            let _ = to_all.send(ToAgent::Data(id, data)).await;
                        }
                        Err(e) => {
                            tracing::error!("Error reading from connection: {}", e);
                            let _ = to_all.send(ToAgent::Closed(id)).await;
                            break;
                        }
                    }
                },
            from_agent = recv.recv() => {
                match from_agent {
                    Some(ToClient::Data(_, data)) => {
                        if let Err(_err) = conn.write_all(&data).await {
                            let _ = to_all.send(ToAgent::Closed(id)).await;
                        }
                    }
                    Some(ToClient::Closed(_)) | None => {
                        break;
                    }
                }
            }
        }
    }
}
