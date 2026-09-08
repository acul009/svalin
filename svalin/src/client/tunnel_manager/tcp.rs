use std::{collections::HashMap, time::Duration};

use serde::{Deserialize, Serialize};
use svalin_rpc::rpc::command::dispatcher::CommandDispatcher;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    select,
    sync::mpsc,
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

pub struct TcpTunnelDispatcher {
    pub listener: TcpListener,
    pub cancel: CancellationToken,
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

impl CommandDispatcher for TcpTunnelDispatcher {
    type Output = ();

    type Error = anyhow::Error;

    type Request = String;

    fn key() -> String {
        "forward-tcp".into()
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

        loop {
            select! {
                _ = self.cancel.cancelled() => {
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
