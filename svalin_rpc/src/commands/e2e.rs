use std::sync::Arc;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use quinn::rustls::server::danger::ClientCertVerifier;
use svalin_pki::{
    Certificate, PermCredentials,
    verifier::{KnownCertificateVerifier, exact::ExactVerififier},
};
use tokio_util::sync::CancellationToken;
use tracing::debug;

use crate::{
    permissions::PermissionHandler,
    rpc::{
        command::{
            dispatcher::{DispatcherError, TakeableCommandDispatcher},
            handler::{HandlerCollection, TakeableCommandHandler},
        },
        peer::Peer,
        session::{Session, SessionDispatchError},
    },
    transport::{
        combined_transport::CombinedTransport,
        session_transport::{SessionTransportReader, SessionTransportWriter},
        tls_transport::{TlsClientError, TlsTransport},
    },
};

fn e2e_key() -> String {
    "e2e".into()
}

pub struct E2EHandler<P, V>
where
    P: PermissionHandler,
{
    credentials: PermCredentials,
    handler_collection: HandlerCollection<P>,
    verifier: Arc<V>,
}

impl<P, V> E2EHandler<P, V>
where
    P: PermissionHandler,
{
    pub fn new(
        credentials: PermCredentials,
        handler_collection: HandlerCollection<P>,
        verifier: Arc<V>,
    ) -> Self {
        Self {
            credentials,
            handler_collection,
            verifier,
        }
    }
}

#[async_trait]
impl<P, V> TakeableCommandHandler for E2EHandler<P, V>
where
    P: PermissionHandler,
    V: ClientCertVerifier + 'static,
{
    type Request = ();

    fn key() -> String {
        e2e_key()
    }

    async fn handle(
        &self,
        session: &mut Option<Session>,
        _: Self::Request,
        cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        if let Some(session_ready) = session.take() {
            let (read, write, _) = session_ready.destructure_transport();

            let tls_transport = TlsTransport::server(
                CombinedTransport::new(read, write),
                self.verifier.clone(),
                &self.credentials,
            )
            .await?;

            let peer = tls_transport.peer().clone();

            let (read, write) = tokio::io::split(tls_transport);

            // TODO: after verifying this, set the correct peer
            let session = Session::new(Box::new(read), Box::new(write), peer);

            session.handle(&self.handler_collection, cancel).await
        } else {
            Err(anyhow!("no session given"))
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum E2EDispatchError {
    #[error("error creating TLS client: {0}")]
    TlsError(TlsClientError),
}

pub struct E2EDispatcher<'b> {
    pub peer: Certificate,
    pub credentials: &'b PermCredentials,
}

#[async_trait]
impl<'b> TakeableCommandDispatcher for E2EDispatcher<'b> {
    type Output = (
        Box<dyn SessionTransportReader>,
        Box<dyn SessionTransportWriter>,
    );
    type InnerError = E2EDispatchError;

    type Request = ();

    fn key() -> String {
        e2e_key()
    }

    fn get_request(&self) -> Self::Request {
        ()
    }

    async fn dispatch(
        self,
        session: &mut Option<Session>,
        _: Self::Request,
    ) -> Result<Self::Output, DispatcherError<Self::InnerError>> {
        if let Some(session_ready) = session.take() {
            debug!("encrypting session");

            let (read, write, _) = session_ready.destructure_transport();
            let tls_transport = TlsTransport::client(
                CombinedTransport::new(read, write),
                // ExactVerififier::new(self.peer.clone()).to_tls_verifier(),
                ExactVerififier::new(self.peer.clone()).to_tls_verifier(),
                self.credentials,
            )
            .await
            .map_err(E2EDispatchError::TlsError)?;

            let (read, write) = tokio::io::split(tls_transport);

            Ok((Box::new(read), Box::new(write)))
        } else {
            Err(DispatcherError::NoneSession)
        }
    }
}
