use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
pub use command::{CommandHandler, HandlerCollection};

pub use client::Client;
pub use connection::{Connection, DirectConnection};
pub use server::Server;
pub use session::{Session, SessionOpen};
pub use skip_verify::SkipServerVerification;

mod client;
mod command;
mod connection;
mod object_stream;
mod ping;
mod server;
mod session;
mod skip_verify;

#[cfg(test)]
mod test;

// pub struct Client {
//     endpoint: quinn::Endpoint,
//     addr: SocketAddr,
// }

// impl Client {
//     pub fn new(addr: SocketAddr) -> Result<Client> {
//         let mut endpoint = quinn::Endpoint::client("[::]:0".parse()?)?;

//         let crypto = rustls::ClientConfig::builder()
//             .with_safe_defaults()
//             .with_custom_certificate_verifier(skip_verify::SkipServerVerification::new())
//             .with_no_client_auth();

//         let client_config = quinn::ClientConfig::new(Arc::new(crypto));

//         endpoint.set_default_client_config(client_config);

//         Ok(Client { endpoint, addr })
//     }

//     pub async fn ping(&mut self) -> Result<()> {
//         let conn = self.endpoint.connect(self.addr, "localhost")?.await?;

//         println!("Connection established, creating data stream");

//         let (mut send, mut recv) = conn.open_bi().await?;

//         println!("Data Stream ready");

//         let mut buff = [0u8; 1024];

//         loop {
//             let ping = SystemTime::now().duration_since(UNIX_EPOCH)?.as_micros();
//             let mut msg = serde_json::to_string(&ping)?;
//             msg.push('\n');
//             send.write_all(msg.as_bytes()).await?;

//             let pong = recv.read(&mut buff).await?;
//             let sent = std::str::from_utf8(&buff[0..pong.unwrap()])?.parse::<u128>()?;
//             let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_micros();
//             let diff = now - sent;
//             println!("diff: {}", diff);
//             tokio::time::sleep(Duration::from_millis(1000)).await;
//         }
//     }
// }
