use std::{pin::Pin, sync::Arc};

use crate::rustls;
use crate::rustls::{client::danger::ServerCertVerifier, server::danger::ClientCertVerifier};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use svalin_pki::PermCredentials;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::{TlsAcceptor, TlsStream};

use super::session_transport::SessionTransport;

pub struct TlsTransport<T>
where
    T: SessionTransport,
{
    tls_stream: TlsStream<T>,
}

impl<T> TlsTransport<T>
where
    T: SessionTransport,
{
    pub async fn client(
        base_transport: T,
        verifier: Arc<dyn ServerCertVerifier>,
        credentials: PermCredentials,
    ) -> Result<Self, (anyhow::Error, T)> {
        let cert_chain = vec![rustls::pki_types::CertificateDer::from(
            credentials.get_certificate().to_der().to_owned(),
        )];

        let key_der =
            rustls::pki_types::PrivateKeyDer::try_from(credentials.get_key_bytes().to_owned());

        if let Err(err) = key_der {
            return Err((anyhow!(err), base_transport));
        }
        let key_der = key_der.unwrap();

        let config = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(verifier)
            .with_client_auth_cert(cert_chain, key_der);

        if let Err(err) = config {
            return Err((anyhow!(err), base_transport));
        }
        let config = config.unwrap();

        let connector = tokio_rustls::TlsConnector::from(Arc::new(config));

        let hostname = rustls::pki_types::ServerName::try_from("todo");

        if let Err(err) = hostname {
            return Err((anyhow!(err), base_transport));
        }
        let hostname = hostname.unwrap();

        let client = connector
            .connect(hostname, base_transport)
            .into_fallible()
            .await;

        match client {
            Err(err) => Err((anyhow!(err.0), err.1)),
            Ok(client) => {
                let tls_stream = TlsStream::Client(client);

                Ok(Self { tls_stream })
            }
        }
    }

    pub async fn server(
        base_transport: T,
        verifier: Arc<dyn ClientCertVerifier>,
        credentials: PermCredentials,
    ) -> Result<Self, (anyhow::Error, T)> {
        let cert_chain = vec![rustls::pki_types::CertificateDer::from(
            credentials.get_certificate().to_der().to_owned(),
        )];

        let key_der =
            rustls::pki_types::PrivateKeyDer::try_from(credentials.get_key_bytes().to_owned());

        if let Err(err) = key_der {
            return Err((anyhow!(err), base_transport));
        }
        let key_der = key_der.unwrap();

        let config = rustls::ServerConfig::builder()
            .with_client_cert_verifier(verifier)
            .with_single_cert(cert_chain, key_der);

        if let Err(err) = config {
            return Err((anyhow!(err), base_transport));
        }
        let config = config.unwrap();

        let acceptor = TlsAcceptor::from(Arc::new(config));

        let server = acceptor.accept(base_transport).into_fallible().await;
        match server {
            Err(err) => Err((anyhow!(err.0), err.1)),
            Ok(server) => {
                let tls_stream = TlsStream::Server(server);

                Ok(Self { tls_stream })
            }
        }
    }

    pub fn derive_key<B>(&self, buffer: B, label: &[u8], context: &[u8]) -> Result<B>
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
}

#[async_trait]
impl<T: SessionTransport> SessionTransport for TlsTransport<T> {}

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
