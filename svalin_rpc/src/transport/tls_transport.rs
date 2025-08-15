use std::{pin::Pin, sync::Arc};

use crate::rpc::peer::Peer;
use crate::rustls;
use crate::rustls::{client::danger::ServerCertVerifier, server::danger::ClientCertVerifier};
use anyhow::Result;
use quinn::rustls::pki_types::InvalidDnsNameError;
use svalin_pki::{Certificate, CertificateParseError, Credential};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::{TlsAcceptor, TlsStream};

use super::session_transport::SessionTransport;

pub struct TlsTransport<T>
where
    T: SessionTransport,
{
    tls_stream: TlsStream<T>,
    peer: Peer,
}

#[derive(Debug, thiserror::Error)]
pub enum TlsClientError {
    #[error("error parsing key DER: {0}")]
    ParseKeyDerError(String),
    #[error("error creating config: {0}")]
    CreateConfigError(rustls::Error),
    #[error("error parsing hostname: {0}")]
    ParseHostError(InvalidDnsNameError),
    #[error("error creating connector: {0}")]
    CreateConnectorError(std::io::Error),
    #[error("peer did not provide a certificate")]
    MissingCertificateError,
    #[error("error parsing certificate: {0}")]
    CertificateParseError(#[from] CertificateParseError),
}

#[derive(Debug, thiserror::Error)]
pub enum TlsServerError {
    #[error("error parsing key DER: {0}")]
    ParseKeyDerError(String),
    #[error("error creating config: {0}")]
    CreateConfigError(rustls::Error),
    #[error("error parsing hostname: {0}")]
    ParseHostError(InvalidDnsNameError),
    #[error("error creating connector: {0}")]
    AcceptConnectionError(std::io::Error),
    #[error("peer did not provide a certificate")]
    MissingCertificateError,
    #[error("error parsing certificate: {0}")]
    CertificateParseError(#[from] CertificateParseError),
}

#[derive(Debug, thiserror::Error)]
pub enum TlsDeriveKeyError {
    #[error("error deriving key: {0}")]
    RustlsError(#[from] rustls::Error),
}

impl<T> TlsTransport<T>
where
    T: SessionTransport,
{
    pub async fn client(
        base_transport: T,
        verifier: Arc<dyn ServerCertVerifier>,
        credentials: &Credential,
    ) -> Result<Self, TlsClientError> {
        let cert_chain = vec![rustls::pki_types::CertificateDer::from(
            credentials.get_certificate().to_der().to_owned(),
        )];

        let key_der = credentials.keypair().rustls_private_key();

        let config = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(verifier)
            .with_client_auth_cert(cert_chain, key_der)
            .map_err(TlsClientError::CreateConfigError)?;

        let connector = tokio_rustls::TlsConnector::from(Arc::new(config));

        // todo
        let hostname = rustls::pki_types::ServerName::try_from("todo")
            .map_err(TlsClientError::ParseHostError)?;

        let client = connector
            .connect(hostname, base_transport)
            .await
            .map_err(TlsClientError::CreateConnectorError)?;

        let der = client
            .get_ref()
            .1
            .peer_certificates()
            .ok_or(TlsClientError::MissingCertificateError)?
            .first()
            .ok_or(TlsClientError::MissingCertificateError)?;

        let cert = Certificate::from_der(der.to_vec())?;

        let tls_stream = TlsStream::Client(client);

        Ok(Self {
            tls_stream,
            peer: Peer::Certificate(cert),
        })
    }

    pub async fn server(
        base_transport: T,
        verifier: Arc<dyn ClientCertVerifier>,
        credentials: &Credential,
    ) -> Result<Self, TlsServerError> {
        let cert_chain = vec![rustls::pki_types::CertificateDer::from(
            credentials.get_certificate().to_der().to_owned(),
        )];

        let key_der = credentials.keypair().rustls_private_key();

        let config = rustls::ServerConfig::builder()
            .with_client_cert_verifier(verifier)
            .with_single_cert(cert_chain, key_der)
            .map_err(TlsServerError::CreateConfigError)?;

        let acceptor = TlsAcceptor::from(Arc::new(config));

        let server = acceptor
            .accept(base_transport)
            .await
            .map_err(TlsServerError::AcceptConnectionError)?;

        let der = server
            .get_ref()
            .1
            .peer_certificates()
            .ok_or(TlsServerError::MissingCertificateError)?
            .first()
            .ok_or(TlsServerError::MissingCertificateError)?;

        let cert = Certificate::from_der(der.to_vec())?;

        let tls_stream = TlsStream::Server(server);

        Ok(Self {
            tls_stream,
            peer: Peer::Certificate(cert),
        })
    }

    pub fn derive_key<B>(
        &self,
        buffer: B,
        label: &[u8],
        context: &[u8],
    ) -> Result<B, TlsDeriveKeyError>
    where
        B: AsMut<[u8]>,
    {
        match &self.tls_stream {
            TlsStream::Client(client) => {
                let (_transport, connection) = client.get_ref();
                Ok(connection.export_keying_material(buffer, label, Some(context))?)
            }
            TlsStream::Server(server) => {
                let (_transport, connection) = server.get_ref();
                Ok(connection.export_keying_material(buffer, label, Some(context))?)
            }
        }
    }

    pub fn peer(&self) -> &Peer {
        &self.peer
    }
}

impl<T: SessionTransport> AsyncRead for TlsTransport<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.tls_stream).poll_read(cx, buf)
    }
}

impl<T: SessionTransport> AsyncWrite for TlsTransport<T> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.tls_stream).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.tls_stream).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.tls_stream).poll_shutdown(cx)
    }
}
