use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use quinn::rustls::server::danger::ClientCertVerifier;
use svalin_pki::{
    verifier::{exact::ExactVerififier, KnownCertificateVerifier, Verifier},
    Certificate, PermCredentials,
};
use tracing::debug;

use crate::{
    permissions::PermissionHandler,
    rpc::{
        command::{
            dispatcher::TakeableCommandDispatcher,
            handler::{HandlerCollection, TakeableCommandHandler},
        },
        peer::Peer,
        session::Session,
    },
    transport::{combined_transport::CombinedTransport, tls_transport::TlsTransport},
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

    async fn handle(&self, session: &mut Option<Session>, _: Self::Request) -> anyhow::Result<()> {
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

            session.handle(&self.handler_collection).await
        } else {
            Err(anyhow!("no session given"))
        }
    }
}

pub struct E2EDispatcher<'b, T> {
    pub peer: Certificate,
    pub credentials: &'b PermCredentials,
    pub nested_dispatch: T,
}

#[async_trait]
impl<'b, D> TakeableCommandDispatcher for E2EDispatcher<'b, D>
where
    D: TakeableCommandDispatcher,
{
    type Output = D::Output;

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
    ) -> Result<Self::Output> {
        if let Some(session_ready) = session.take() {
            debug!("encrypting session");

            let (read, write, _) = session_ready.destructure_transport();
            let tls_transport = TlsTransport::client(
                CombinedTransport::new(read, write),
                // ExactVerififier::new(self.peer.clone()).to_tls_verifier(),
                ExactVerififier::new(self.peer.clone()).to_tls_verifier(),
                self.credentials,
            )
            .await?;

            let (read, write) = tokio::io::split(tls_transport);

            let session_ready = Session::new(
                Box::new(read),
                Box::new(write),
                Peer::Certificate(self.peer),
            );

            session_ready.dispatch(self.nested_dispatch).await
        } else {
            Err(anyhow!("no session given"))
        }
    }
}
