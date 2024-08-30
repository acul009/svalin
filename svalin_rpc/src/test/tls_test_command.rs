use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{
    self as svalin_rpc,
    transport::tls_transport::{self, TlsTransport},
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::future::ok;
use svalin_macros::rpc_dispatch;
use svalin_pki::{Keypair, PermCredentials};
use tracing::error;

use crate::rpc::{command::handler::CommandHandler, session::Session};

pub struct TlsTestCommandHandler {
    credentials: PermCredentials,
}

impl TlsTestCommandHandler {
    pub fn new() -> Result<Self> {
        let credentials = Keypair::generate()?.to_self_signed_cert()?;

        Ok(Self { credentials })
    }
}

fn tls_test_key() -> String {
    "tls_test".into()
}

#[async_trait]
impl CommandHandler for TlsTestCommandHandler {
    fn key(&self) -> String {
        tls_test_key()
    }

    async fn handle(&self, session: &mut Option<Session>) -> anyhow::Result<()> {
        session
            .replace_transport(|direct_transport| async {
                let credentials = Keypair::generate().unwrap().to_self_signed_cert().unwrap();

                let tls_transport = TlsTransport::server(
                    direct_transport,
                    crate::verifiers::skip_verify::SkipClientVerification::new(),
                    &credentials,
                )
                .await;

                match tls_transport {
                    Ok(tls_transport) => Box::new(tls_transport),
                    Err(err) => err.1,
                }
            })
            .await;

        let ping: u64 = session.read_object().await?;
        session.write_object(&ping).await?;

        Ok(())
    }
}

#[rpc_dispatch(tls_test_key())]
pub async fn tls_test(session: &mut Session) -> Result<()> {
    session
        .replace_transport(|direct_transport| async {
            let credentials = Keypair::generate().unwrap().to_self_signed_cert().unwrap();

            let tls_transport = TlsTransport::client(
                direct_transport,
                crate::verifiers::skip_verify::SkipServerVerification::new(),
                &credentials,
            )
            .await;

            match tls_transport {
                Ok(tls_transport) => Box::new(tls_transport),
                Err(err) => {
                    panic!("{}", err.0);
                    err.1
                }
            }
        })
        .await;

    let ping = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_nanos();

    session.write_object(&ping).await?;

    let pong: u128 = session.read_object().await?;

    let now: u128 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_nanos();

    let diff = Duration::from_nanos((now - pong).try_into()?);

    println!("TLS-Ping: {:?}", diff);

    Ok(())
}
