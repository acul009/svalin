use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_pki::{
    CertificateType, EncryptedObject, SpkiHash,
    mls::{
        key_package::UnverifiedKeyPackage,
        provider::ExportedMlsStore,
        transport_types::{MessageToMemberTransport, MessageToServerTransport},
    },
};
use svalin_rpc::rpc::{command::handler::CommandHandler, peer::Peer, session::Session};
use svalin_store::{
    client_store::persistent,
    server_store::{KeyPackageStore, MessageStore, UserStore},
};
use tokio::select;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::server::MlsServer;

pub struct UpdateMls {}

pub struct UpdateMlsHandler {
    user_store: Arc<UserStore>,
    message_store: Arc<MessageStore>,
    key_package_store: Arc<KeyPackageStore>,
    mls: Arc<MlsServer>,
    user_lock: Mutex<HashMap<SpkiHash, Arc<tokio::sync::Mutex<()>>>>,
}

#[async_trait]
impl CommandHandler for UpdateMlsHandler {
    type Request = ();

    fn key() -> String {
        "update-mls".into()
    }
    async fn handle(
        &self,
        session: &mut Session,
        request: Self::Request,
        cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let Peer::Certificate(cert) = session.peer() else {
            anyhow::bail!("Only certificate peers can update mls")
        };
        if cert.certificate_type() != CertificateType::UserSession {
            anyhow::bail!("Only user sessions can update mls")
        };
        let user = cert.issuer().clone();
        let arc = self
            .user_lock
            .lock()
            .unwrap()
            .entry(user.clone())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone();
        let Ok(_guard) = arc.try_lock() else {
            anyhow::bail!("User is already updating mls")
        };

        let Some(user_data) = self.user_store.get_user(&user).await? else {
            anyhow::bail!("User not found")
        };
        let saved_state = SavedState {
            mls_store: user_data.mls_store,
            persistent_data: user_data.persistent_data,
        };
        session.write_object(&saved_state).await?;

        let mut sub = self.message_store.subscribe(user.clone()).await;
        let old_messages = self.message_store.load_all_for(&user).await?;

        for message in old_messages {
            session
                .write_object(&OldUpdate::Message(message.0, message.1))
                .await?;
        }
        session.write_object(&OldUpdate::UpToDate).await?;
        // Switch to live updates
        let package_count = self.key_package_store.count_key_packages(&user).await?;
        session
            .write_object(&LiveUpdate::KeyPackageCount(package_count))
            .await?;

        loop {
            select! {
                _ = cancel.cancelled() => {
                    break;
                }
                message = sub.recv() => {
                    if let Some((id, message)) = message {
                        session.write_object(&LiveUpdate::Message(id, message)).await?;
                    } else {
                        session.write_object(&LiveUpdate::Goodbye).await?;
                        break;
                    }
                }
                update = session.read_object::<ToServer>() => {
                    let Ok(update) = update else {
                        anyhow::bail!("Failed to read update: {update:?}");
                    };
                    match update {
                        ToServer::StateUpdate { mls_store, persistent_data, key_packages, aknowledged, messages } => todo!(),
                        ToServer::Goodbye => todo!(),
                    }
                }
            }
        }

        todo!()
    }
}

#[derive(Serialize, Deserialize)]
struct SavedState {
    mls_store: ExportedMlsStore,
    persistent_data: EncryptedObject<persistent::State>,
}

#[derive(Serialize, Deserialize, Debug)]
enum OldUpdate {
    Message(Uuid, MessageToMemberTransport),
    UpToDate,
}

#[derive(Serialize, Deserialize, Debug)]
enum LiveUpdate {
    Message(Uuid, Arc<MessageToMemberTransport>),
    KeyPackageCount(u64),
    Goodbye,
}

#[derive(Serialize, Deserialize, Debug)]
enum ToServer {
    StateUpdate {
        mls_store: ExportedMlsStore,
        persistent_data: EncryptedObject<persistent::State>,
        key_packages: Vec<UnverifiedKeyPackage>,
        aknowledged: Vec<Uuid>,
        // I thought about sending those seperate, but sending a message moves the ratchet,
        // so it's important that these are synced with the mls_store update
        messages: Vec<MessageToServerTransport>,
    },
    Goodbye,
}
