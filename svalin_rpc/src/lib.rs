use core::net::SocketAddr;
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Ok, Result};
use futures::stream::FuturesUnordered;
use quinn::{AcceptBi, RecvStream, SendStream};
use rustls::PrivateKey;
use tokio::task::JoinSet;

mod command;
mod object_stream;
mod ping;
mod session;
mod skip_verify;

use session::Session;

pub struct Server {
    endpoint: quinn::Endpoint,
    open_connections: JoinSet<()>,
}

impl Server {
    pub fn new(addr: SocketAddr) -> Result<Self> {
        let endpoint = Server::create_endpoint(addr)?;

        Ok(Server {
            endpoint,
            open_connections: JoinSet::new(),
        })
    }

    fn create_endpoint(addr: SocketAddr) -> Result<quinn::Endpoint> {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let cert_der = cert.serialize_der().unwrap();
        let priv_key = cert.serialize_private_key_der();
        let priv_key = rustls::PrivateKey(priv_key);
        let cert_chain = vec![rustls::Certificate(cert_der.clone())];

        let config = quinn::ServerConfig::with_crypto(Server::create_crypto(cert_chain, priv_key)?);

        let endpoint = quinn::Endpoint::server(config, addr)?;

        Ok(endpoint)
    }

    fn create_crypto(
        cert_chain: Vec<rustls::Certificate>,
        priv_key: PrivateKey,
    ) -> Result<Arc<rustls::ServerConfig>> {
        let mut cfg = rustls::ServerConfig::builder()
            .with_safe_default_cipher_suites()
            .with_safe_default_kx_groups()
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .with_no_client_auth()
            .with_single_cert(cert_chain, priv_key)?;
        cfg.max_early_data_size = u32::MAX;
        Ok(Arc::new(cfg))
    }

    pub async fn run(&mut self) -> Result<()> {
        println!("starting server");
        while let Some(conn) = self.endpoint.accept().await {
            println!("connection incoming");
            let fut = Server::handle_connection(conn);
            self.open_connections.spawn(async move {
                println!("spawn successful");
                if let Err(e) = fut.await {
                    print!("Error: {}", e);
                }
                println!("connection handled");
            });
            println!("Waiting for next connection");
        }
        todo!()
    }

    async fn handle_connection(conn: quinn::Connecting) -> Result<()> {
        println!("waiting for connection to get ready...");

        let conn = conn.await?;

        let peer_cert = match conn.peer_identity() {
            None => Ok(None),
            Some(ident) => match ident.downcast::<rustls::Certificate>() {
                core::result::Result::Ok(cert) => Ok(Some(cert)),
                Err(_) => Err(anyhow!("Failed to get legitimate identity")),
            },
        }?;

        if let Some(cert) = peer_cert {
            println!("client cert:\n{:?}", cert.as_ref());
        } else {
            println!("client did not provide cert")
        }

        println!("connection established");

        let conn = Connection::new(conn);
        conn.handle().await?;

        Ok(())
    }

    async fn handle_stream(stream: (SendStream, RecvStream)) -> Result<()> {
        println!("echoing data stream");
        let (mut send, mut recv) = stream;
        let mut buffer = [0u8; 1024];
        loop {
            let byte_count = recv.read(&mut buffer).await?;
            if let Some(count) = byte_count {
                send.write_all(&buffer[0..count]).await?;
            }
        }
    }
}

pub struct Connection {
    conn: quinn::Connection,
    open_streams: JoinSet<()>,
}

impl Connection {
    fn new(conn: quinn::Connection) -> Self {
        Connection {
            conn,
            open_streams: JoinSet::new(),
        }
    }

    async fn handle(mut self) -> Result<()> {
        println!("waiting for incoming data stream");
        loop {
            match self.conn.accept_bi().await {
                Err(e) => {
                    while let Some(res) = self.open_streams.join_next().await {}
                    Err(e)
                }

                core::result::Result::Ok((send, recv)) => {
                    println!("data stream incoming!");
                    let session = Session::new(recv, send);
                    let fut = Server::handle_stream(stream);
                    self.open_streams.spawn(async move {
                        if let Err(e) = fut.await {
                            print!("Error: {}", e);
                        }
                    });
                    core::result::Result::Ok(())
                }
            }?;
        }
    }
}

pub struct Client {
    endpoint: quinn::Endpoint,
    addr: SocketAddr,
}

impl Client {
    pub fn new(addr: SocketAddr) -> Result<Client> {
        let mut endpoint = quinn::Endpoint::client("[::]:0".parse()?)?;

        let crypto = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_custom_certificate_verifier(skip_verify::SkipServerVerification::new())
            .with_no_client_auth();

        let client_config = quinn::ClientConfig::new(Arc::new(crypto));

        endpoint.set_default_client_config(client_config);

        Ok(Client {
            endpoint: endpoint,
            addr: addr,
        })
    }

    pub async fn ping(&mut self) -> Result<()> {
        let conn = self
            .endpoint
            .connect("127.0.0.1:1234".parse()?, "localhost")?
            .await?;

        println!("Connection established, creating data stream");

        let (mut send, mut recv) = conn.open_bi().await?;

        println!("Data Stream ready");

        let mut buff = [0u8; 1024];

        loop {
            let ping = SystemTime::now().duration_since(UNIX_EPOCH)?.as_micros();
            let mut msg = serde_json::to_string(&ping)?;
            msg.push('\n');
            send.write_all(msg.as_bytes()).await?;

            let pong = recv.read(&mut buff).await?;
            let sent = std::str::from_utf8(&buff[0..pong.unwrap()])?.parse::<u128>()?;
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_micros();
            let diff = now - sent;
            println!("diff: {}", diff);
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
    }
}
