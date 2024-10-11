use anyhow::{anyhow, Result};
use async_trait::async_trait;
use svalin_pki::{
    verifier::{exact::ExactVerififier, KnownCertificateVerifier},
    Certificate, PermCredentials,
};

use crate::{
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

pub struct E2EHandler {
    credentials: PermCredentials,
    handler_collection: HandlerCollection,
}

impl E2EHandler {
    pub fn new(credentials: PermCredentials, handler_collection: HandlerCollection) -> Self {
        Self {
            credentials,
            handler_collection,
        }
    }
}

#[async_trait]
impl TakeableCommandHandler for E2EHandler {
    fn key(&self) -> String {
        e2e_key()
    }

    async fn handle(&self, session: &mut Option<Session>) -> anyhow::Result<()> {
        if let Some(session_ready) = session.take() {
            let (read, write, _) = session_ready.destructure_transport();

            let tls_transport = TlsTransport::server(
                CombinedTransport::new(read, write),
                // TODO: actually fucking verify the connecting peer
                crate::verifiers::skip_verify::SkipClientVerification::new(),
                &self.credentials,
            )
            .await?;

            let (read, write) = tokio::io::split(tls_transport);

            // TODO: after verifying this, set the correct peer
            let session_ready = Session::new(Box::new(read), Box::new(write), Peer::Anonymous);

            session_ready.handle(&self.handler_collection).await
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

    fn key(&self) -> String {
        e2e_key()
    }
    async fn dispatch(self, session: &mut Option<Session>) -> Result<Self::Output> {
        if let Some(session_ready) = session.take() {
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
