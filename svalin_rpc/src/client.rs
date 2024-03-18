use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use quinn::crypto;
use svalin_pki::PermCredentials;



pub struct Client;

impl Client {
    pub fn connect(addr: SocketAddr, identity: Option<PermCredentials>, verifier: Arc<dyn rustls::client::ServerCertVerifier>) -> Result<Client> {
        let endpoint = quinn::Endpoint::client(addr)?;



        let builder  = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_custom_certificate_verifier(verifier);

        let conf = match identity {
            Some(id) => {
                builder.with_client_auth_cert(vec![rustls::Certificate(id.get_certificate().to_der().to_owned())], key_der)
            },
            None => {
                builder.with_no_client_auth()
            }
            
        };
        Ok(Client {})
    }
}