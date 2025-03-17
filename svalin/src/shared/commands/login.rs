use anyhow::{Result, anyhow};
use async_trait::async_trait;
use aucpace::{AuCPaceServer, ClientMessage, ServerMessage};
use curve25519_dalek::RistrettoPoint;
use serde::{
    Deserialize, Serialize,
    de::{self, Expected},
};
use svalin_pki::{ArgonParams, Keypair};
use svalin_rpc::{
    rpc::{
        command::handler::{CommandHandler, TakeableCommandHandler},
        peer::Peer,
        server,
        session::{self, Session},
    },
    transport::{combined_transport::CombinedTransport, tls_transport::TlsTransport},
    verifiers::skip_verify::SkipClientVerification,
};
use tokio_util::sync::CancellationToken;

use crate::server::user_store::UserStore;

#[derive(Serialize, Deserialize)]
struct LoginAttempt {
    password_hash: Vec<u8>,
    current_totp: String,
}

#[derive(Serialize, Deserialize)]
pub struct LoginSuccess {
    pub encrypted_credentials: Vec<u8>,
}

pub struct LoginHandler {
    user_store: UserStore,
    fake_seed: Vec<u8>,
}

impl LoginHandler {
    pub fn new(user_store: UserStore, fake_seed: Vec<u8>) -> Self {
        Self {
            user_store,
            fake_seed,
        }
    }
}

const NONCE_LENGTH: usize = 16;

#[derive(Debug, thiserror::Error)]
pub enum MessageTransformError {
    #[error("wrong input variant")]
    WrongInputVariant,
}

#[derive(Serialize, Deserialize)]
struct Nonce([u8; NONCE_LENGTH]);
impl TryFrom<ServerMessage<'_, NONCE_LENGTH>> for Nonce {
    type Error = MessageTransformError;

    fn try_from(value: ServerMessage<'_, NONCE_LENGTH>) -> std::result::Result<Self, Self::Error> {
        match value {
            ServerMessage::Nonce(nonce) => Ok(Nonce(nonce)),
            _ => Err(MessageTransformError::WrongInputVariant),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct StrongUsername {
    username: Vec<u8>,
    blinded: RistrettoPoint,
}

#[derive(Serialize, Deserialize)]
struct PublicKey(RistrettoPoint);
impl TryFrom<ServerMessage<'_, NONCE_LENGTH>> for PublicKey {
    type Error = MessageTransformError;

    fn try_from(value: ServerMessage<'_, NONCE_LENGTH>) -> std::result::Result<Self, Self::Error> {
        match value {
            ServerMessage::PublicKey(pubkey) => Ok(PublicKey(pubkey)),
            _ => Err(MessageTransformError::WrongInputVariant),
        }
    }
}

struct Authenticator([u8; 64]);
impl TryFrom<ServerMessage<'_, NONCE_LENGTH>> for Authenticator {
    type Error = MessageTransformError;

    fn try_from(value: ServerMessage<'_, NONCE_LENGTH>) -> std::result::Result<Self, Self::Error> {
        match value {
            ServerMessage::Authenticator(authenticator) => Ok(Authenticator(authenticator)),
            _ => Err(MessageTransformError::WrongInputVariant),
        }
    }
}
impl Serialize for Authenticator {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(&self.0)
    }
}
impl<'de> Deserialize<'de> for Authenticator {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_byte_buf(AuthenticatorVisitor)
    }
}
struct AuthenticatorVisitor;

impl<'de> de::Visitor<'de> for AuthenticatorVisitor {
    type Value = Authenticator;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("an authenticator")
    }

    fn visit_byte_buf<E>(self, v: Vec<u8>) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Authenticator(v.try_into().map_err(|vec: Vec<u8>| {
            de::Error::invalid_length(vec.len(), &"64 bytes")
        })?))
    }

    fn visit_bytes<E>(self, v: &[u8]) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_byte_buf(v.to_vec())
    }
}

#[async_trait]
impl TakeableCommandHandler for LoginHandler {
    type Request = ();

    fn key() -> String {
        "login".to_string()
    }

    async fn handle(
        &self,
        session: &mut Option<Session>,
        _request: Self::Request,
        _cancel: CancellationToken,
    ) -> Result<()> {
        if let Some(mut session) = session.take() {
            let username: String = session.read_object().await?;

            // ===== Establish TLS Connection and generate common secret =====

            let (read, write, _) = session.destructure_transport();

            let temp_credentials = Keypair::generate().unwrap().to_self_signed_cert()?;

            let tls_transport = TlsTransport::server(
                CombinedTransport::new(read, write),
                SkipClientVerification::new(),
                &temp_credentials,
            )
            .await?;

            let mut key_material = [0u8; 32];
            let key_material = tls_transport
                .derive_key(&mut key_material, b"login", username.as_bytes())
                .unwrap();

            let (read, write) = tokio::io::split(tls_transport);

            let mut session = Session::new(Box::new(read), Box::new(write), Peer::Anonymous);

            // ===== Get User =====

            let user = self.user_store.get_user_by_username(username.as_ref())?;

            // ===== SSID Establishment =====
            let mut pake_server: AuCPaceServer<_, _, NONCE_LENGTH> =
                aucpace::Server::new(Default::default());

            let (pake_server, server_nonce) = pake_server.begin();

            let server_nonce = Nonce::try_from(server_nonce)?;

            session.write_object(&server_nonce).await?;

            // ===== Augmentation Layer =====
            let client_nonce: Nonce = session.read_object().await?;

            let pake_server = pake_server.agree_ssid(client_nonce.0);

            let strong_username: StrongUsername = session.read_object().await?;

            let (pake_server, client_info) = pake_server
                .generate_client_info_strong(
                    strong_username.username,
                    strong_username.blinded,
                    &self.user_store,
                    password_hash::rand_core::OsRng,
                )
                .map_err(|err| anyhow!(err))?;

            session.write_object(&client_info).await?;

            // ===== CPace substep =====
            let (pake_server, public_key) = pake_server.generate_public_key(key_material);
            let server_public_key = PublicKey::try_from(public_key)?;

            session.write_object(&server_public_key).await?;

            let client_public_key: PublicKey = session.read_object().await?;

            let pake_server = pake_server
                .receive_client_pubkey(client_public_key.0)
                .map_err(|err| anyhow!(err))?;

            // ===== Explicit Mutual Authentication =====

            let client_authenticator: Authenticator = session.read_object().await?;
            let (key, server_authenticator) = pake_server
                .receive_client_authenticator(client_authenticator.0)
                .map_err(|err| anyhow!(err))?;

            let server_authenticator = Authenticator::try_from(server_authenticator)?;

            session.write_object(&server_authenticator).await?;

            todo!("do something with this key...")
        } else {
            Err(anyhow!("tried executing commandhandler with None"))
        }
    }
}
