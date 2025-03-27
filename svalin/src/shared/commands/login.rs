use std::{str, sync::Arc};

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use aucpace::{AuCPaceClient, AuCPaceServer, ClientMessage, ServerMessage};
use curve25519_dalek::{
    RistrettoPoint,
    digest::{generic_array::GenericArray, typenum},
};
use password_hash::{
    ParamsString,
    rand_core::{OsRng, RngCore},
};
use serde::{
    Deserialize, Serialize,
    de::{self},
};
use svalin_pki::{
    ArgonCost, ArgonParams, Certificate, DecryptError, EncryptError, EncryptedData, Keypair,
    ParamsStringParseError, ToSelfSingedError, argon2::Argon2, sha2::Sha512,
};
use svalin_rpc::{
    rpc::{
        command::{
            dispatcher::{DispatcherError, TakeableCommandDispatcher},
            handler::{PermissionPrecursor, TakeableCommandHandler},
        },
        peer::Peer,
        session::{Session, SessionReadError, SessionWriteError},
    },
    transport::{
        combined_transport::CombinedTransport,
        tls_transport::{TlsClientError, TlsDeriveKeyError, TlsTransport},
    },
    verifiers::skip_verify::{SkipClientVerification, SkipServerVerification},
};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tracing::debug;

use crate::{
    permissions::Permission,
    server::user_store::{UserStore, serde_paramsstring},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginSuccess {
    pub encrypted_credentials: Vec<u8>,
    pub root_cert: Certificate,
    pub server_cert: Certificate,
}

impl From<&PermissionPrecursor<(), LoginHandler>> for Permission {
    fn from(_value: &PermissionPrecursor<(), LoginHandler>) -> Self {
        Permission::AnonymousOnly
    }
}

pub struct LoginHandler {
    user_store: Arc<UserStore>,
    root_cert: Certificate,
    server_cert: Certificate,
}

impl LoginHandler {
    pub fn new(
        user_store: Arc<UserStore>,
        root_cert: Certificate,
        server_cert: Certificate,
    ) -> Self {
        Self {
            user_store,
            root_cert,
            server_cert,
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
    fn generate() -> Self {
        let mut nonce = [0u8; NONCE_LENGTH];
        password_hash::rand_core::OsRng.fill_bytes(&mut nonce);
        Nonce(nonce)
    }
}

#[derive(Serialize, Deserialize)]
struct StrongUsername {
    username: Vec<u8>,
    blinded: RistrettoPoint,
}

impl TryFrom<ClientMessage<'_, NONCE_LENGTH>> for StrongUsername {
    type Error = MessageTransformError;
    fn try_from(value: ClientMessage<'_, NONCE_LENGTH>) -> std::result::Result<Self, Self::Error> {
        match value {
            ClientMessage::StrongUsername { username, blinded } => Ok(StrongUsername {
                username: username.to_vec(),
                blinded,
            }),
            _ => Err(MessageTransformError::WrongInputVariant),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct ClientInfo {
    /// J from the protocol definition
    group: String,

    /// X from the protocol definition
    x_pub: RistrettoPoint,

    /// the blinded salt used with the PBKDF
    blinded_salt: RistrettoPoint,

    /// the parameters for the PBKDF used - sigma from the protocol definition
    #[serde(with = "serde_paramsstring")]
    hash_params: ParamsString,
}

impl TryFrom<ServerMessage<'_, NONCE_LENGTH>> for ClientInfo {
    type Error = MessageTransformError;
    fn try_from(value: ServerMessage<'_, NONCE_LENGTH>) -> std::result::Result<Self, Self::Error> {
        match value {
            ServerMessage::StrongAugmentationInfo {
                group,
                x_pub,
                blinded_salt,
                pbkdf_params,
            } => Ok(ClientInfo {
                group: group.to_string(),
                x_pub,
                blinded_salt,
                hash_params: pbkdf_params,
            }),
            _ => Err(MessageTransformError::WrongInputVariant),
        }
    }
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

impl TryFrom<ClientMessage<'_, NONCE_LENGTH>> for PublicKey {
    type Error = MessageTransformError;

    fn try_from(value: ClientMessage<'_, NONCE_LENGTH>) -> std::result::Result<Self, Self::Error> {
        match value {
            ClientMessage::PublicKey(pubkey) => Ok(PublicKey(pubkey)),
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
            // TODO: Really important!!!
            // implement rate limiting!!!

            let tls_server_nonce = Nonce::generate();

            session.write_object(&tls_server_nonce).await?;

            let tls_client_nonce: Nonce = session.read_object().await?;

            let tls_combined_nonce: Vec<u8> = tls_server_nonce
                .0
                .into_iter()
                .chain(tls_client_nonce.0.into_iter())
                .collect();

            // ===== Establish TLS Connection and generate common secret =====

            let (read, write, _) = session.destructure_transport();

            let temp_credentials = Keypair::generate()
                .to_self_signed_cert()
                .context("Failed to generate temporary credentials")?;

            let tls_transport = TlsTransport::server(
                CombinedTransport::new(read, write),
                SkipClientVerification::new(),
                &temp_credentials,
            )
            .await
            .context("Failed to establish TLS connection")?;

            let mut key_material = [0u8; 32];
            let key_material = tls_transport
                .derive_key(&mut key_material, b"login", &tls_combined_nonce)
                .context("Failed to derive key")?;

            let (read, write) = tokio::io::split(tls_transport);

            let mut session = Session::new(Box::new(read), Box::new(write), Peer::Anonymous);

            debug!("server session recreated");

            // ===== SSID Establishment =====
            let mut pake_server: AuCPaceServer<_, _, NONCE_LENGTH> =
                aucpace::Server::new(Default::default());

            debug!("sending server nonce");

            let (pake_server, server_nonce) = pake_server.begin();

            session
                .write_object(&Nonce::try_from(server_nonce)?)
                .await
                .context("Failed to write server nonce")?;

            debug!("reading for client nonce");

            // ===== Augmentation Layer =====
            let client_nonce: Nonce = session
                .read_object()
                .await
                .context("Failed to read client nonce")?;

            let pake_server = pake_server.agree_ssid(client_nonce.0);

            debug!("reading for strong username");

            let strong_username: StrongUsername = session
                .read_object()
                .await
                .context("Failed to read strong username")?;

            let username = strong_username.username.clone();

            let (pake_server, client_info) = pake_server
                .generate_client_info_strong(
                    strong_username.username,
                    strong_username.blinded,
                    self.user_store.as_ref(),
                    password_hash::rand_core::OsRng,
                )
                .map_err(|err| anyhow!(err))?;

            let mut client_info = ClientInfo::try_from(client_info)?;
            if client_info.hash_params.is_empty() {
                client_info.hash_params = ArgonCost::strong().get_params().try_into()?;
            }
            debug!("hash params: {}", &client_info.hash_params);

            session
                .write_object(&client_info)
                .await
                .context("Failed to write client info")?;

            // ===== CPace substep =====
            let (pake_server, public_key) = pake_server.generate_public_key(key_material);
            let server_public_key = PublicKey::try_from(public_key)?;

            session.write_object(&server_public_key).await?;

            let client_public_key: PublicKey = session
                .read_object()
                .await
                .context("Failed to read client public key")?;

            let pake_server = pake_server
                .receive_client_pubkey(client_public_key.0)
                .map_err(|err| anyhow!(err))
                .context("Failed to receive client public key")?;

            // ===== Explicit Mutual Authentication =====

            let client_authenticator: Authenticator = session
                .read_object()
                .await
                .context("Failed to read client authenticator")?;

            let client_auth_result =
                pake_server.receive_client_authenticator(client_authenticator.0);

            let key = match client_auth_result {
                Ok((key, server_authenticator)) => {
                    let server_authenticator = Authenticator::try_from(server_authenticator)?;
                    session
                        .write_object::<Result<_, ()>>(&Ok(server_authenticator))
                        .await
                        .context("Failed to write server authenticator")?;
                    key
                }
                Err(err) => {
                    session
                        .write_object::<Result<Authenticator, ()>>(&Err(()))
                        .await
                        .context("Failed to inform client about authentication failure")?;

                    return Err(anyhow!(err).context("failed to authenticate"));
                }
            };

            // ===== TOTP =====

            let user = self
                .user_store
                .get_user_by_username(&username)?
                .ok_or_else(|| anyhow!("failed to get user by username"))?;

            let totp_encrypted: Vec<u8> =
                session.read_object().await.context("Failed to read totp")?;

            let totp: String =
                EncryptedData::decrypt_object_with_key(&totp_encrypted, key_to_array(key))?;

            let totp_success = user.totp_secret.check_current(&totp)?;

            session
                .write_object(&totp_success)
                .await
                .context("Failed to write totp success")?;

            if !totp_success {
                return Err(anyhow!("failed to verify totp"));
            }

            let success = LoginSuccess {
                encrypted_credentials: user.encrypted_credentials,
                root_cert: self.root_cert.clone(),
                server_cert: self.server_cert.clone(),
            };

            let encrypted_success =
                EncryptedData::encrypt_object_with_key(&success, key_to_array(key))?;

            session
                .write_object(&encrypted_success)
                .await
                .context("Failed to write encrypted success")?;

            Ok(())
        } else {
            Err(anyhow!("tried executing commandhandler with None"))
        }
    }
}

pub struct Login {
    pub username: Vec<u8>,
    pub password: Vec<u8>,
    pub totp: String,
}

#[derive(Debug, thiserror::Error)]
pub enum LoginDispatcherError {
    #[error("error writing client nonce: {0}")]
    WriteClientNonceError(SessionWriteError),
    #[error("error reading server nonce: {0}")]
    ReadServerNonceError(SessionReadError),
    #[error("error creating temporary credentials: {0}")]
    TempCredentialError(ToSelfSingedError),
    #[error("error initializing TLS: {0}")]
    TlsClientError(TlsClientError),
    #[error("error deriving key: {0}")]
    DeriveKeyError(TlsDeriveKeyError),
    #[error("error transforming message: {0}")]
    MessageTransformError(MessageTransformError),
    #[error("error writing username: {0}")]
    WriteUsernameError(SessionWriteError),
    #[error("error reading client info: {0}")]
    ReadClientInfoError(SessionReadError),
    #[error("error receiving channel: {0}")]
    ChannelRecvError(oneshot::error::RecvError),
    #[error("error sending channel")]
    ChannelSendError,
    #[error("error joining task: {0}")]
    JoinError(tokio::task::JoinError),
    #[error("error parsing params: {0}")]
    ParamsParseError(ParamsStringParseError),
    #[error("error in aucpace: {0}")]
    AucPaceError(aucpace::Error),
    #[error("error writing client public key: {0}")]
    ClientPublicKeyWriteError(SessionWriteError),
    #[error("error reading server public key: {0}")]
    ServerPublicKeyReadError(SessionReadError),
    #[error("error writing client authenticator: {0}")]
    ClientAuthenticatorWriteError(SessionWriteError),
    #[error("error reading server authenticator: {0}")]
    ServerAuthenticatorReadError(SessionReadError),
    #[error("error encrypting data: {0}")]
    EncryptError(EncryptError),
    #[error("error decrypting data: {0}")]
    DecryptError(DecryptError),
    #[error("error writing totp: {0}")]
    WriteTotpError(SessionWriteError),
    #[error("error reading totp response: {0}")]
    ReadTotpResponseError(SessionReadError),
    #[error("totp invalid")]
    InvalidTotp,
    #[error("error reading success: {0}")]
    ReadSuccessError(SessionReadError),
    #[error("wrong password")]
    WrongPassword,
}

#[async_trait]
impl TakeableCommandDispatcher for Login {
    type Output = LoginSuccess;
    type InnerError = LoginDispatcherError;

    type Request = ();

    fn key() -> String {
        LoginHandler::key()
    }

    fn get_request(&self) -> Self::Request {
        ()
    }

    async fn dispatch(
        self,
        session: &mut Option<Session>,
        _: Self::Request,
    ) -> Result<Self::Output, DispatcherError<Self::InnerError>> {
        if let Some(mut session) = session.take() {
            let tls_client_nonce = Nonce::generate();

            session
                .write_object(&tls_client_nonce)
                .await
                .map_err(LoginDispatcherError::WriteClientNonceError)?;

            let tls_server_nonce: Nonce = session
                .read_object()
                .await
                .map_err(LoginDispatcherError::ReadServerNonceError)?;

            let tls_combined_nonce: Vec<u8> = tls_server_nonce
                .0
                .into_iter()
                .chain(tls_client_nonce.0.into_iter())
                .collect();

            // ===== TLS Initialization =====
            let (read, write, _) = session.destructure_transport();

            let credentials = Keypair::generate()
                .to_self_signed_cert()
                .map_err(LoginDispatcherError::TempCredentialError)?;

            let tls_transport = TlsTransport::client(
                CombinedTransport::new(read, write),
                SkipServerVerification::new(),
                &credentials,
            )
            .await
            .map_err(LoginDispatcherError::TlsClientError)?;

            debug!("tls transport created");

            let mut key_material = [0u8; 32];
            let key_material = tls_transport
                .derive_key(&mut key_material, b"login", &tls_combined_nonce)
                .map_err(LoginDispatcherError::DeriveKeyError)?;

            let (read, write) = tokio::io::split(tls_transport);
            let mut session = Session::new(Box::new(read), Box::new(write), Peer::Anonymous);

            debug!("session recreated");

            // ===== SSID Establishment =====
            let mut client = AuCPaceClient::<Sha512, Argon2, _, NONCE_LENGTH>::new(OsRng);

            debug!("sending client nonce");

            let (client, client_nonce) = client.begin();
            session
                .write_object(
                    &Nonce::try_from(client_nonce)
                        .map_err(LoginDispatcherError::MessageTransformError)?,
                )
                .await
                .map_err(LoginDispatcherError::WriteClientNonceError)?;

            debug!("receiving server nonce");

            // Receive server nonce
            let server_nonce: Nonce = session
                .read_object()
                .await
                .map_err(LoginDispatcherError::ReadServerNonceError)?;
            let client = client.agree_ssid(server_nonce.0);

            let (username_send, username_recv) = oneshot::channel();
            let (client_info_send, client_info_recv) = oneshot::channel();

            debug!("starting augmentation task");

            // ===== Augmentation Layer =====
            // Running is a blocking task, so the hashing doesn't cause issues
            let blocking_handle = tokio::task::spawn_blocking(move || {
                let username = self.username.clone();
                let password = self.password.clone();
                let (client, strong_username) =
                    client.start_augmentation_strong(&username, &password, &mut OsRng);

                username_send
                    .send(
                        StrongUsername::try_from(strong_username)
                            .map_err(LoginDispatcherError::MessageTransformError)?,
                    )
                    .map_err(|_| LoginDispatcherError::ChannelSendError)?;

                // Receive augmentation info
                let client_info: ClientInfo = client_info_recv
                    .blocking_recv()
                    .map_err(LoginDispatcherError::ChannelRecvError)?;

                // ===== CPace substep =====
                let hasher = ArgonCost::try_from(client_info.hash_params)
                    .map_err(LoginDispatcherError::ParamsParseError)?
                    .get_argon_hasher();

                client
                    .generate_cpace_alloc(
                        client_info.x_pub,
                        client_info.blinded_salt,
                        hasher.params().clone(),
                        hasher,
                    )
                    .map_err(LoginDispatcherError::AucPaceError)
            });

            debug!("sending strong username");

            session
                .write_object(
                    &username_recv
                        .await
                        .map_err(LoginDispatcherError::ChannelRecvError)?,
                )
                .await
                .map_err(LoginDispatcherError::WriteUsernameError)?;

            debug!("receiving client info");

            let client_info: ClientInfo = session
                .read_object()
                .await
                .map_err(LoginDispatcherError::ReadClientInfoError)?;

            client_info_send
                .send(client_info)
                .map_err(|_| LoginDispatcherError::ChannelSendError)?;

            let client = blocking_handle
                .await
                .map_err(LoginDispatcherError::JoinError)??;

            debug!("sending client public key");

            let (client, client_key) = client.generate_public_key(key_material, &mut OsRng);

            session
                .write_object(
                    &PublicKey::try_from(client_key)
                        .map_err(LoginDispatcherError::MessageTransformError)?,
                )
                .await
                .map_err(LoginDispatcherError::ClientPublicKeyWriteError)?;

            debug!("receiving server public key");

            let server_key: PublicKey = session
                .read_object()
                .await
                .map_err(LoginDispatcherError::ServerPublicKeyReadError)?;

            // ===== Explicit Mutual Authentication =====

            debug!("sending client authenticator");

            let (client, client_authenticator) = client
                .receive_server_pubkey(server_key.0)
                .map_err(LoginDispatcherError::AucPaceError)?;

            session
                .write_object(
                    &Authenticator::try_from(client_authenticator)
                        .map_err(LoginDispatcherError::MessageTransformError)?,
                )
                .await
                .map_err(LoginDispatcherError::ClientAuthenticatorWriteError)?;

            debug!("receiving server authenticator");

            let server_authenticator = session
                .read_object::<Result<Authenticator, ()>>()
                .await
                .map_err(LoginDispatcherError::ServerAuthenticatorReadError)?
                .map_err(|_| LoginDispatcherError::WrongPassword)?;

            let key = client
                .receive_server_authenticator(server_authenticator.0)
                .map_err(LoginDispatcherError::AucPaceError)?;

            let totp = EncryptedData::encrypt_object_with_key(&self.totp, key_to_array(key))
                .map_err(LoginDispatcherError::EncryptError)?;

            session
                .write_object(&totp)
                .await
                .map_err(LoginDispatcherError::WriteTotpError)?;

            let totp_success: bool = session
                .read_object()
                .await
                .map_err(LoginDispatcherError::ReadTotpResponseError)?;

            if !totp_success {
                return Err(LoginDispatcherError::InvalidTotp.into());
            }

            let encrypted_success: Vec<u8> = session
                .read_object()
                .await
                .map_err(LoginDispatcherError::ReadSuccessError)?;

            let success: LoginSuccess =
                EncryptedData::decrypt_object_with_key(&encrypted_success, key_to_array(key))
                    .map_err(LoginDispatcherError::DecryptError)?;

            Ok(success)
        } else {
            Err(DispatcherError::NoneSession)
        }
    }
}

fn key_to_array(key: GenericArray<u8, typenum::U64>) -> [u8; 32] {
    [
        key[0], key[1], key[2], key[3], key[4], key[5], key[6], key[7], key[8], key[9], key[10],
        key[11], key[12], key[13], key[14], key[15], key[16], key[17], key[18], key[19], key[20],
        key[21], key[22], key[23], key[24], key[25], key[26], key[27], key[28], key[29], key[30],
        key[31],
    ]
}
