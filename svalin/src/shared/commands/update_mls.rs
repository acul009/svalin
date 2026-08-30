use std::{
    collections::HashMap,
    mem,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_pki::{
    CertificateType, Credential, EncryptedObject, EncryptionKey, SpkiHash, TrustStoreVerifier,
    mls::{
        client::MessageDataContent,
        key_package::UnverifiedKeyPackage,
        provider::{ExportedMlsStore, SvalinStorage},
        transport_types::{MessageToMemberTransport, MessageToServerTransport},
    },
};
use svalin_rpc::rpc::{
    command::{dispatcher::CommandDispatcher, handler::CommandHandler},
    peer::Peer,
    session::Session,
};
use svalin_store::{
    client_store::persistent::{self, SvalinMetaInfo},
    server_store::{KeyPackageStore, MessageStore, UserStore},
};
use tokio::{
    select,
    sync::{mpsc, oneshot},
    time::Instant,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    message_streaming::client::ClientStateHandle, mls::MlsClient,
    remote_key_retriever::RemoteKeyRetriever, server::MlsServer,
};

const WANTED_KEY_PACKAGES: u64 = 100;

pub struct UpdateMls {
    pub mls_updates: mpsc::Receiver<MlsUpdate>,
    pub user_credential: Credential,
    pub encryption_key: EncryptionKey,
    pub key_retriever: RemoteKeyRetriever,
    pub verifier: TrustStoreVerifier,
    pub client_state: ClientStateHandle,
    pub cancel: CancellationToken,
    pub up_to_date: oneshot::Sender<()>,
}

pub enum MlsUpdate {
    UpdateMetaInfo(SpkiHash, SvalinMetaInfo),
}

impl CommandDispatcher for UpdateMls {
    type Output = ();

    type Error = anyhow::Error;

    type Request = ();

    fn key() -> String {
        UpdateMlsHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &()
    }

    async fn dispatch(mut self, session: &mut Session) -> Result<Self::Output, Self::Error> {
        let saved: SavedState = session.read_object().await?;
        let (store, export_handle) = SvalinStorage::import(saved.mls_store, &self.encryption_key)?;
        let mls = MlsClient::new(
            self.user_credential.clone(),
            store,
            self.key_retriever.clone(),
            self.verifier.clone(),
        )?;
        let saved_persistent = saved.persistent_data.decrypt(&self.encryption_key)?;
        let (snapshot, _) = self.client_state.subscribe().await?;
        let snapshot = snapshot.persistent().devices();

        // Update data from that saved on server
        for (device, state) in saved_persistent.devices() {
            if let Some(my_device) = snapshot.get(device) {
                if let Some(saved_meta) = state.meta_info() {
                    let mut update_meta = true;
                    if let Some(my_meta) = my_device.meta_info() {
                        update_meta = saved_meta.updated_at > my_meta.updated_at;
                    }
                    if update_meta {
                        self.client_state
                            .persistent_update(persistent::Message::UpdateMetaInfo(
                                device.clone(),
                                saved_meta.clone(),
                            ))
                            .await?;
                    }
                }
                if let Some(saved_report) = state.report() {
                    let mut update_report = true;
                    if let Some(my_report) = my_device.report() {
                        update_report = saved_report.system_report.generated_at
                            > my_report.system_report.generated_at;
                    }
                    if update_report {
                        self.client_state
                            .persistent_update(persistent::Message::UpdateSystemReport(
                                device.clone(),
                                saved_report.clone(),
                            ))
                            .await?;
                    }
                }
            }
        }
        let mut aknowledged = Vec::new();

        while let OldUpdate::Message(uuid, message) = session.read_object().await? {
            let message_data = mls.handle_message(&message).await?;
            match message_data.content {
                MessageDataContent::Report(spki_hash, report) => {
                    self.client_state
                        .persistent_update(persistent::Message::UpdateSystemReport(
                            spki_hash, report,
                        ))
                        .await?
                }
                MessageDataContent::MetaInfo(spki_hash, meta_info) => {
                    self.client_state
                        .persistent_update(persistent::Message::UpdateMetaInfo(
                            spki_hash, meta_info,
                        ))
                        .await?
                }
                MessageDataContent::Internal => (),
            }

            aknowledged.push(uuid);
        }

        let mut key_packages = Vec::new();
        while key_packages.len() as u64 + saved.key_package_count < WANTED_KEY_PACKAGES {
            let key_package = mls.create_key_package().await?;
            key_packages.push(key_package.to_unverified());
        }

        let (snapshot, _) = self.client_state.subscribe().await?;
        let persistent_data =
            EncryptedObject::encrypt(snapshot.persistent(), &self.encryption_key)?;

        let update = ToServer::StateUpdate {
            mls_store: export_handle.export(&self.encryption_key)?,
            persistent_data,
            key_packages,
            aknowledged,
            messages: Vec::new(),
        };
        session.write_object(&update).await?;

        let _ = self.up_to_date.send(());

        // Now in live mode
        let mut messages = Vec::new();
        let mut key_packages = Vec::new();
        let mut aknowledged = Vec::new();

        loop {
            let timeout =
                !messages.is_empty() || !key_packages.is_empty() || !aknowledged.is_empty();
            select! {
                _ = self.cancel.cancelled() => {
                    session.write_object(&ToServer::Goodbye).await?;
                    break;
                }
                _ = tokio::time::sleep(Duration::from_secs(3)), if timeout => {
                    let (snapshot, _) = self.client_state.subscribe().await?;
                    let persistent_data =
                        EncryptedObject::encrypt(snapshot.persistent(), &self.encryption_key)?;

                    let update = ToServer::StateUpdate {
                        mls_store: export_handle.export(&self.encryption_key)?,
                        persistent_data,
                        key_packages: mem::take(&mut key_packages),
                        aknowledged: mem::take(&mut aknowledged),
                        messages: mem::take(&mut messages),
                    };
                    session.write_object(&update).await?;
                }
                update = self.mls_updates.recv(), if self.mls_updates.is_closed() => {
                    let Some(update) = update else {
                        continue;
                    };
                    match update {
                        MlsUpdate::UpdateMetaInfo(spki_hash, svalin_meta_info) => {
                            if let Some(message) = mls.create_meta_group_if_missing(spki_hash.clone()).await? {
                                messages.push(message);
                            }
                            let message = mls.send_meta_info(spki_hash, svalin_meta_info).await?;
                            messages.push(message);
                        },
                    }
                }
                update = session.read_object::<LiveUpdate>() => {
                    let Ok(update) = update else {
                        anyhow::bail!("Failed to read update: {update:?}");
                    };
                    match update {
                        LiveUpdate::Message(uuid, message) => {
                            let message_data = mls.handle_message(&message).await?;
                            match message_data.content {
                                MessageDataContent::Report(spki_hash, report) => {
                                    self.client_state
                                        .persistent_update(persistent::Message::UpdateSystemReport(
                                            spki_hash, report,
                                        ))
                                        .await?
                                }
                                MessageDataContent::MetaInfo(spki_hash, meta_info) => {
                                    self.client_state
                                        .persistent_update(persistent::Message::UpdateMetaInfo(
                                            spki_hash, meta_info,
                                        ))
                                        .await?
                                }
                                MessageDataContent::Internal => (),
                            }

                            aknowledged.push(uuid);
                        },
                        LiveUpdate::KeyPackageCount(key_package_count) => {
                            while key_packages.len() as u64 + key_package_count < WANTED_KEY_PACKAGES {
                                let key_package = mls.create_key_package().await?;
                                key_packages.push(key_package.to_unverified());
                            }
                        },
                        LiveUpdate::Goodbye => break,
                    }
                }
            }
        }

        Ok(())
    }
}

pub struct UpdateMlsHandler {
    user_store: Arc<UserStore>,
    message_store: Arc<MessageStore>,
    key_package_store: Arc<KeyPackageStore>,
    mls: Arc<MlsServer>,
    user_lock: Mutex<HashMap<SpkiHash, Arc<tokio::sync::Mutex<()>>>>,
}
impl UpdateMlsHandler {
    pub(crate) fn new(
        user_store: Arc<UserStore>,
        message_store: Arc<MessageStore>,
        key_package_store: Arc<KeyPackageStore>,
        mls: Arc<MlsServer>,
    ) -> Self {
        Self {
            user_store,
            message_store,
            key_package_store,
            mls,
            user_lock: Mutex::new(HashMap::new()),
        }
    }
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
        _request: Self::Request,
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
            key_package_count: self.key_package_store.count_key_packages(&user).await?,
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

        let mut key_package_interval = tokio::time::interval_at(
            Instant::now() + Duration::from_secs(30),
            Duration::from_secs(30),
        );
        key_package_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            select! {
                _ = cancel.cancelled() => {
                    session.write_object(&LiveUpdate::Goodbye).await?;
                    break;
                }
                _ = key_package_interval.tick() => {
                    let count = self.key_package_store.count_key_packages(&user).await?;
                    session.write_object(&LiveUpdate::KeyPackageCount(count)).await?;
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
                        ToServer::StateUpdate { mls_store, persistent_data, key_packages, aknowledged, messages: incoming_messages } => {
                            self.user_store.update_mls_data(&user, mls_store, persistent_data).await?;
                            for key_package in key_packages {
                                let key_package = self.mls.verify_key_package(key_package, &user).await?;
                                self.key_package_store.add_key_package(key_package).await?;
                            }
                            self.message_store.aknowledge_messages(&user, &aknowledged).await?;
                            for message in incoming_messages {
                                let to_send = self.mls.process_message(message).await?;
                                for message in to_send {
                                    self.message_store.add_message(message).await?;
                                }
                            }
                        },
                        ToServer::Goodbye => break,
                    }
                }
            }
        }

        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct SavedState {
    mls_store: ExportedMlsStore,
    persistent_data: EncryptedObject<persistent::State>,
    key_package_count: u64,
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
