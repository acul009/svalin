use aucpace::{AuCPaceClient, AuCPaceServer, ClientMessage, ServerMessage};
use serde::{Deserialize, Serialize, de};
use std::pin::Pin;
use svalin_pki::{
    ArgonCost, ParamsStringParseError, Sha512,
    argon2::{
        Argon2,
        password_hash::{
            self, ParamsString, SaltString,
            rand_core::{OsRng, RngCore},
        },
    },
    curve25519_dalek::{
        RistrettoPoint,
        digest::{generic_array::GenericArray, typenum},
    },
    serde_paramsstring, serde_saltstring,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::{debug, instrument};

use crate::{
    rpc::{
        peer::Peer,
        session::{Session, SessionReadError, SessionWriteError},
    },
    transport::{
        session_transport::SessionTransport,
        tls_transport::{TlsClientError, TlsServerError, TlsTransport},
    },
};

pub struct AucPaceTransport<T>
where
    T: SessionTransport,
{
    tls_transport: TlsTransport<T>,
}

#[derive(Debug, thiserror::Error)]
pub enum AucPaceClientError {
    #[error("TLS client error: {0}")]
    TlsClientError(#[from] TlsClientError),
    #[error("Session write error: {0}")]
    SessionWrite(#[from] SessionWriteError),
    #[error("Session read error: {0}")]
    SessionRead(#[from] SessionReadError),
    #[error("Message transform error: {0}")]
    TransformError(#[from] MessageTransformError),
    #[error("AucPace error: {0}")]
    AucPaceError(aucpace::Error),
    #[error("Join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),
    #[error("password hash error: {0}")]
    PasswordHashError(#[from] svalin_pki::password_hash::Error),
    #[error("authentication failed: {0}")]
    AuthenticationFailed(aucpace::Error),
    #[error("params string parse error: {0}")]
    ParamsStringParseError(#[from] ParamsStringParseError),
    #[error("wrong password")]
    WrongPassword,
}

impl From<aucpace::Error> for AucPaceClientError {
    fn from(err: aucpace::Error) -> Self {
        Self::AucPaceError(err)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AucPaceServerError {
    #[error("TLS server error: {0}")]
    TlsServerError(#[from] TlsServerError),
    #[error("Session write error: {0}")]
    SessionWrite(#[from] SessionWriteError),
    #[error("Session read error: {0}")]
    SessionRead(#[from] SessionReadError),
    #[error("Message transform error: {0}")]
    TransformError(#[from] MessageTransformError),
    #[error("AucPace error: {0}")]
    AucPaceError(aucpace::Error),
    #[error("Join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),
    #[error("password hash error: {0}")]
    PasswordHashError(#[from] svalin_pki::password_hash::Error),
    #[error("authentication failed: {0}")]
    AuthenticationFailed(aucpace::Error),
}

impl From<aucpace::Error> for AucPaceServerError {
    fn from(err: aucpace::Error) -> Self {
        Self::AucPaceError(err)
    }
}

impl<T> AucPaceTransport<T>
where
    T: SessionTransport + 'static,
{
    #[instrument(skip_all)]
    pub async fn client(transport: T, password: Vec<u8>) -> Result<Self, AucPaceClientError> {
        let mut session = Session::new(Box::new(transport), Peer::Anonymous);

        // ===== SSID Establishment =====
        let mut client = AuCPaceClient::<Sha512, Argon2, _, NONCE_LENGTH>::new(OsRng);

        let (client, client_nonce) = client.begin();
        session
            .write_object(&Nonce::try_from(client_nonce)?)
            .await?;

        // Receive server nonce
        let server_nonce: Nonce = session.read_object().await?;
        let client = client.agree_ssid(server_nonce.to_array());

        // ===== Augmentation Layer =====

        let client_info: ClientInfo = session.read_object().await?;
        let hasher = ArgonCost::try_from(client_info.hash_params)?.get_argon_hasher();
        let client = tokio::task::spawn_blocking(move || {
            let (client, _username_message) = client.start_augmentation(USERNAME, &password);
            client.generate_cpace_alloc(
                client_info.x_pub,
                &client_info.salt,
                hasher.params().clone(),
                hasher,
            )
        })
        .await??;

        // ===== CPace substep =====
        let client_nonce = Nonce::generate();
        session.write_object(&client_nonce).await?;
        let server_nonce: Nonce = session.read_object().await?;
        let combined = server_nonce.combine(client_nonce);

        let (client, client_key) = client.generate_public_key(combined, &mut OsRng);
        let client_key = PublicKey::try_from(client_key)?;
        session.write_object(&client_key).await?;
        let server_key: PublicKey = session.read_object().await?;

        // ===== Explicit Mutual Authentication =====

        let (client, client_authenticator) = client.receive_server_pubkey(server_key.0)?;
        let client_authenticator = Authenticator::try_from(client_authenticator)?;
        session.write_object(&client_authenticator).await?;

        let server_authenticator = session
            .read_object::<Result<Authenticator, ()>>()
            .await?
            .map_err(|_| AucPaceClientError::WrongPassword)?;
        let key = client.receive_server_authenticator(server_authenticator.0)?;
        let key = key_to_array(key);

        // ===== Create TLS Tunnel =====
        let (transport, _) = session.destructure();
        let transport = transport.into_any().downcast::<T>().unwrap();

        let tls_transport = TlsTransport::client_preshared(*transport, key).await?;

        Ok(Self { tls_transport })
    }

    #[instrument(skip_all)]
    pub async fn server(transport: T, password: Vec<u8>) -> Result<Self, AucPaceServerError> {
        let mut session = Session::new(Box::new(transport), Peer::Anonymous);

        // ===== Pseudo-Registration =====
        let mut pake_client: AuCPaceClient<Sha512, Argon2, OsRng, NONCE_LENGTH> =
            AuCPaceClient::new(OsRng);

        let hasher = ArgonCost::basic().get_argon_hasher();
        let (salt, params, verifier) = match tokio::task::spawn_blocking(move || {
            pake_client.register_alloc(USERNAME, password, hasher.params().clone(), hasher)
        })
        .await??
        {
            ClientMessage::Registration {
                username: _,
                salt,
                params,
                verifier,
            } => (salt, params, verifier),
            _ => unreachable!(),
        };
        let db = PseudoDatabase::new(verifier, salt, params);

        // ===== SSID Establishment =====
        debug!("Starting SSID establishment");
        let mut pake_server: AuCPaceServer<_, _, NONCE_LENGTH> =
            aucpace::Server::new(password_hash::rand_core::OsRng);

        let (pake_server, server_nonce) = pake_server.begin();
        session
            .write_object(&Nonce::try_from(server_nonce)?)
            .await?;

        // ===== Augmentation Layer =====
        debug!("Starting augmentation layer");
        let client_nonce: Nonce = session.read_object().await?;

        let pake_server = pake_server.agree_ssid(client_nonce.to_array());

        let (pake_server, client_info) = tokio::task::spawn_blocking(move || {
            pake_server.generate_client_info(USERNAME, &db, password_hash::rand_core::OsRng)
        })
        .await?;

        let mut client_info = ClientInfo::try_from(client_info)?;
        if client_info.hash_params.is_empty() {
            client_info.hash_params = ArgonCost::strong().get_params().try_into()?;
        }
        session.write_object(&client_info).await?;

        // ===== CPace substep =====
        debug!("Starting CPace layer");
        let server_nonce = Nonce::generate();
        session.write_object(&server_nonce).await?;
        let client_nonce: Nonce = session.read_object().await?;
        let combined = server_nonce.combine(client_nonce);

        let (pake_server, public_key) = pake_server.generate_public_key(combined);
        let server_public_key = PublicKey::try_from(public_key)?;
        session.write_object(&server_public_key).await?;
        let client_key: PublicKey = session.read_object().await?;

        // ===== Explicit Mutual Authentication =====
        debug!("Starting explicit mutual authentication");

        let pake_server = pake_server.receive_client_pubkey(client_key.0)?;
        let client_authenticator: Authenticator = session.read_object().await?;

        let client_auth_result = pake_server.receive_client_authenticator(client_authenticator.0);

        let key = match client_auth_result {
            Ok((key, server_authenticator)) => {
                let server_authenticator = Authenticator::try_from(server_authenticator)?;
                session
                    .write_object::<Result<_, ()>>(&Ok(server_authenticator))
                    .await?;
                key_to_array(key)
            }
            Err(err) => {
                session
                    .write_object::<Result<Authenticator, ()>>(&Err(()))
                    .await?;

                return Err(AucPaceServerError::AuthenticationFailed(err));
            }
        };

        // ===== Create TLS Tunnel =====
        debug!("Creating TLS tunnel");
        let (transport, _) = session.destructure();
        let transport = transport.into_any().downcast::<T>().unwrap();

        let tls_transport = TlsTransport::server_preshared(*transport, key).await?;

        Ok(Self { tls_transport })
    }
}

const USERNAME: &[u8] = b"aucpace";

struct PseudoDatabase {
    data: (RistrettoPoint, SaltString, ParamsString),
}

impl PseudoDatabase {
    pub fn new(verifier: RistrettoPoint, salt: SaltString, params: ParamsString) -> Self {
        Self {
            data: (verifier, salt, params),
        }
    }
}

impl aucpace::Database for PseudoDatabase {
    type PasswordVerifier = RistrettoPoint;

    fn lookup_verifier(
        &self,
        _username: &[u8],
    ) -> Option<(
        Self::PasswordVerifier,
        password_hash::SaltString,
        password_hash::ParamsString,
    )> {
        Some(self.data.clone())
    }

    fn store_verifier(
        &mut self,
        _username: &[u8],
        _salt: password_hash::SaltString,
        _uad: Option<&[u8]>,
        _verifier: Self::PasswordVerifier,
        _params: password_hash::ParamsString,
    ) {
        unimplemented!()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MessageTransformError {
    #[error("wrong input variant")]
    WrongInputVariant,
}

pub const NONCE_LENGTH: usize = 16;

#[derive(Serialize, Deserialize)]
pub struct Nonce([u8; NONCE_LENGTH]);
impl TryFrom<ServerMessage<'_, NONCE_LENGTH>> for Nonce {
    type Error = MessageTransformError;

    fn try_from(value: ServerMessage<'_, NONCE_LENGTH>) -> std::result::Result<Self, Self::Error> {
        match value {
            ServerMessage::Nonce(nonce) => Ok(Nonce(nonce)),
            _ => Err(MessageTransformError::WrongInputVariant),
        }
    }
}

impl TryFrom<ClientMessage<'_, NONCE_LENGTH>> for Nonce {
    type Error = MessageTransformError;

    fn try_from(value: ClientMessage<'_, NONCE_LENGTH>) -> std::result::Result<Self, Self::Error> {
        match value {
            ClientMessage::Nonce(nonce) => Ok(Nonce(nonce)),
            _ => Err(MessageTransformError::WrongInputVariant),
        }
    }
}

impl Nonce {
    pub fn generate() -> Self {
        let mut nonce = [0u8; NONCE_LENGTH];
        password_hash::rand_core::OsRng.fill_bytes(&mut nonce);
        Nonce(nonce)
    }

    pub fn combine(self, other: Self) -> Vec<u8> {
        let mut combined = Vec::with_capacity(NONCE_LENGTH * 2);
        combined.extend_from_slice(&self.0);
        combined.extend_from_slice(&other.0);
        combined
    }

    pub fn to_array(self) -> [u8; NONCE_LENGTH] {
        self.0
    }
}

#[derive(Serialize, Deserialize)]
pub struct PublicKey(pub RistrettoPoint);

impl TryFrom<ServerMessage<'_, NONCE_LENGTH>> for PublicKey {
    type Error = MessageTransformError;

    fn try_from(value: ServerMessage<'_, NONCE_LENGTH>) -> std::result::Result<Self, Self::Error> {
        match value {
            ServerMessage::PublicKey(pubkey) => Ok(PublicKey(pubkey)),
            _ => Err(MessageTransformError::WrongInputVariant),
        }
    }
}

impl TryFrom<ClientMessage<'_, NONCE_LENGTH>> for PublicKey {
    type Error = MessageTransformError;

    fn try_from(value: ClientMessage<'_, NONCE_LENGTH>) -> std::result::Result<Self, Self::Error> {
        match value {
            ClientMessage::PublicKey(pubkey) => Ok(PublicKey(pubkey)),
            _ => Err(MessageTransformError::WrongInputVariant),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ClientInfo {
    /// J from the protocol definition
    pub group: String,

    /// X from the protocol definition
    pub x_pub: RistrettoPoint,

    /// the blinded salt used with the PBKDF
    #[serde(with = "serde_saltstring")]
    pub salt: SaltString,

    /// the parameters for the PBKDF used - sigma from the protocol definition
    #[serde(with = "serde_paramsstring")]
    pub hash_params: ParamsString,
}

impl TryFrom<ServerMessage<'_, NONCE_LENGTH>> for ClientInfo {
    type Error = MessageTransformError;
    fn try_from(value: ServerMessage<'_, NONCE_LENGTH>) -> std::result::Result<Self, Self::Error> {
        match value {
            ServerMessage::AugmentationInfo {
                group,
                x_pub,
                salt,
                pbkdf_params,
            } => Ok(ClientInfo {
                group: group.to_string(),
                x_pub,
                salt,
                hash_params: pbkdf_params,
            }),
            _ => Err(MessageTransformError::WrongInputVariant),
        }
    }
}

pub struct Authenticator(pub [u8; 64]);
impl TryFrom<ServerMessage<'_, NONCE_LENGTH>> for Authenticator {
    type Error = MessageTransformError;

    fn try_from(value: ServerMessage<'_, NONCE_LENGTH>) -> std::result::Result<Self, Self::Error> {
        match value {
            ServerMessage::Authenticator(authenticator) => Ok(Authenticator(authenticator)),
            _ => Err(MessageTransformError::WrongInputVariant),
        }
    }
}

impl TryFrom<ClientMessage<'_, NONCE_LENGTH>> for Authenticator {
    type Error = MessageTransformError;

    fn try_from(value: ClientMessage<'_, NONCE_LENGTH>) -> std::result::Result<Self, Self::Error> {
        match value {
            ClientMessage::Authenticator(authenticator) => Ok(Authenticator(authenticator)),
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

pub fn key_to_array(key: GenericArray<u8, typenum::U64>) -> [u8; 32] {
    [
        key[0], key[1], key[2], key[3], key[4], key[5], key[6], key[7], key[8], key[9], key[10],
        key[11], key[12], key[13], key[14], key[15], key[16], key[17], key[18], key[19], key[20],
        key[21], key[22], key[23], key[24], key[25], key[26], key[27], key[28], key[29], key[30],
        key[31],
    ]
}

impl<T: SessionTransport> AsyncRead for AucPaceTransport<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.tls_transport).poll_read(cx, buf)
    }
}

impl<T: SessionTransport> AsyncWrite for AucPaceTransport<T> {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.tls_transport).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.tls_transport).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.tls_transport).poll_shutdown(cx)
    }
}
