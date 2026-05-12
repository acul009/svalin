use std::{
    collections::HashMap,
    mem,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, anyhow};
use async_trait::async_trait;
use futures::{FutureExt, select};
use serde::{Deserialize, Serialize};
use svalin_client_store::persistent;
use svalin_pki::{
    CertificateType, Credential, EncryptedObject, EncryptionKey, SpkiHash,
    mls::{
        SvalinGroupId,
        client::{MessageDataContent, MlsClient},
        key_package::UnverifiedKeyPackage,
        provider::{ExportedMlsStore, SvalinStorage},
        transport_types::{MessageToMemberTransport, MessageToServerTransport},
    },
};
use svalin_rpc::rpc::command::{dispatcher::CommandDispatcher, handler::CommandHandler};
use svalin_server_store::{KeyPackageStore, MessageStore, UserStore};
use svalin_sysctl::sytem_report::SystemReport;
use tokio::{
    sync::mpsc,
    time::{Instant, sleep_until},
};
use tokio_util::sync::CancellationToken;
use tracing::debug;
use uuid::Uuid;

use crate::{
    client::state::ClientStateUpdate, message_streaming::client::ClientStateHandle,
    remote_key_retriever::RemoteKeyRetriever, server::MlsServer,
    verifier::remote_verifier::RemoteVerifier,
};

pub struct UpdateUserMlsHandler {
    user_store: Arc<UserStore>,
    message_store: Arc<MessageStore>,
    key_package_store: Arc<KeyPackageStore>,
    mls: Arc<MlsServer>,
    user_lock: Mutex<HashMap<SpkiHash, Arc<tokio::sync::Mutex<()>>>>,
}

impl UpdateUserMlsHandler {
    pub fn new(
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
impl CommandHandler for UpdateUserMlsHandler {
    type Request = ();

    fn key() -> String {
        "update_user_mls".into()
    }

    async fn handle(
        &self,
        session: &mut svalin_rpc::rpc::session::Session,
        _request: Self::Request,
        cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let peer = session.peer().certificate()?;
        if peer.certificate_type() != CertificateType::UserSession {
            return Err(anyhow::anyhow!("wrong certificate type, expected session"));
        }
        let user_hash = peer.issuer().clone();

        let user_arc = {
            self.user_lock
                .lock()
                .unwrap()
                .entry(user_hash.clone())
                .or_default()
                .clone()
        };
        let _user_lock = user_arc.lock().await;

        let user = self
            .user_store
            .get_user(&user_hash)
            .await?
            .ok_or_else(|| anyhow!("User not found"))?;

        // Ok, so at this point I need a way to load all messages which should be delivered in order.
        // Once all of them are sent over, the dispatcher should get a signal that we're done, so we'll porbably be sending Options.
        // The Dispatcher should then send an updated version of the user's MlsState to the server.
        // I might want to somehow verify that new MlsState too. Maybe with a SignedObject?
        // Either way, the dispatcher should send both the updated MlsState, but also the ids of all messages which were processed.

        let saved_state = SavedState {
            mls_store: user.mls_store,
            persistent_data: user.persistent_data,
        };
        session.write_object(&saved_state).await?;

        let (send, mut recv) = mpsc::channel(100);
        tokio::spawn(stream_mls_messages(
            user_hash.clone(),
            send,
            self.message_store.clone(),
        ));

        // Yield not implemented yet
        let mut next_key_package_update = Instant::now();

        loop {
            let update = select! {
                received = recv.recv().fuse() => {
                    if let Some((uuid, message)) = received {
                        Update::Message(uuid, message)
                    } else {
                        break;
                    }
                },
                _ = cancel.cancelled().fuse() => {
                    break;
                }
                response = session.read_object::<ToServer>().fuse() => {
                    // Todo: make the object reader / chunk reader cancel save with an internal buffer
                    let response = response?;
                    tracing::info!("user mls initiative: {response:?}");
                    self.handle_response(&user_hash, response).await?;
                    continue;
                }
                _ = sleep_until(next_key_package_update).fuse() => {
                    next_key_package_update = Instant::now() + Duration::from_secs(60);
                    Update::KeyPackageCount(self.key_package_store.count_key_packages(&user_hash).await?)
                }
            };

            session.write_object(&update).await?;

            let response = session.read_object::<ToServer>().await?;
            tracing::debug!("user mls respone: {response:?}");
            self.handle_response(&user_hash, response).await?;
        }

        // explicit drop, so I don't accidentally drop it beforehand
        drop(_user_lock);
        Ok(())
    }
}

impl UpdateUserMlsHandler {
    async fn handle_response(
        &self,
        user_hash: &SpkiHash,
        response: ToServer,
    ) -> anyhow::Result<()> {
        match response {
            ToServer::StateUpdate {
                mls_store,
                persistent_data,
                key_packages,
                aknowledged,
                messages,
            } => {
                for key_package in key_packages {
                    let key_package = self
                        .mls
                        .verify_key_package(key_package, user_hash)
                        .await
                        .context("error verifying key package")?;
                    self.key_package_store.add_key_package(key_package).await?;
                }

                self.user_store
                    .update_mls_data(&user_hash, mls_store, persistent_data)
                    .await?;

                self.message_store
                    .aknowledge_messages(&user_hash, &aknowledged)
                    .await?;

                for message in messages {
                    tracing::debug!("received message from user mls: {message:?}");
                    let to_send = self.mls.process_message(message).await?;
                    tracing::debug!("processing resulted in messages: {to_send:?}");
                    for mut message in to_send {
                        message.remove_receiver(&user_hash);
                        self.message_store.add_message(message).await?;
                    }
                }
            }
            ToServer::OK => {}
        }

        Ok(())
    }
}

async fn stream_mls_messages(
    receiver: SpkiHash,
    sender: mpsc::Sender<(Uuid, Arc<MessageToMemberTransport>)>,
    message_store: Arc<MessageStore>,
) {
    if let Err(e) = stream_mls_messages_inner(receiver, sender, message_store).await {
        tracing::error!("Error streaming mls messages: {}", e);
    }
}

async fn stream_mls_messages_inner(
    receiver: SpkiHash,
    sender: mpsc::Sender<(Uuid, Arc<MessageToMemberTransport>)>,
    message_store: Arc<MessageStore>,
) -> Result<(), anyhow::Error> {
    let current = message_store.load_all_for(&receiver).await?;
    let mut subscription = message_store.subscribe(receiver.clone()).await;
    for (uuid, message) in current {
        if let Err(e) = sender.send((uuid, Arc::new(message))).await {
            anyhow::bail!("Error sending message: {}", e);
        }
    }

    while let Some(message) = subscription.recv().await {
        let _ = sender.send(message).await;
    }

    Ok(())
}

pub struct UpdateUserMls {
    pub key: EncryptionKey,
    pub user_credential: Credential,
    pub key_retriever: RemoteKeyRetriever,
    pub verifier: RemoteVerifier,
    pub session_mls: Arc<MlsClient<RemoteKeyRetriever, RemoteVerifier>>,
    pub state_handle: ClientStateHandle,
    pub cancel: CancellationToken,
}

const WANTED_KEY_PACKAGES: u64 = 100;

impl CommandDispatcher for UpdateUserMls {
    type Output = ();

    type Error = anyhow::Error;

    type Request = ();

    fn key() -> String {
        UpdateUserMlsHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        &()
    }

    async fn dispatch(
        self,
        session: &mut svalin_rpc::rpc::session::Session,
    ) -> Result<Self::Output, Self::Error> {
        debug!("Updating user MLS");

        loop {
            let Some(state) = self
                .cancel
                .run_until_cancelled(session.read_object::<SavedState>())
                .await
            else {
                return Ok(());
            };
            let state = state?;

            let (mls_store, export_handle) = SvalinStorage::import(state.mls_store, &self.key)
                .context("error importing mls storage")?;
            let mut persistent_data = state
                .persistent_data
                .decrypt(&self.key)
                .context("error decrypting persistent data")?;
            let client = MlsClient::new(
                self.user_credential.clone(),
                mls_store,
                self.key_retriever.clone(),
                self.verifier.clone(),
            )
            .context("error building mls client")?;

            let mut aknowledge = Vec::new();
            let mut messages = Vec::new();
            let mut key_packages = Vec::new();
            let mut should_yield = false;
            let mut timeout_duration = Duration::from_secs(3);

            while !should_yield {
                let Some(update) = self
                    .cancel
                    .run_until_cancelled(tokio::time::timeout(
                        timeout_duration,
                        session.read_object::<Update>(),
                    ))
                    .await
                else {
                    return Ok(());
                };

                let send_update;

                if let Ok(update) = update {
                    // No timeout, so other messages might follow directly afterwards
                    let update = update?;
                    // Next timeout should be short, so we can send out updates as soon as possible
                    // A bit of timeout is still good, so we have some debounce.
                    timeout_duration = Duration::from_secs(3);
                    tracing::debug!("user mls update: {update:?}");
                    match update {
                        Update::Message(uuid, message_to_member_transport) => {
                            let handled = client
                                .handle_message::<SystemReport>(&message_to_member_transport)
                                .await?;

                            match handled.content {
                                MessageDataContent::Report(spki_hash, report) => {
                                    persistent_data.update(
                                        persistent::Message::UpdateSystemReport(spki_hash, report),
                                    );
                                }
                                MessageDataContent::Internal => {}
                            }

                            aknowledge.push(uuid);
                            send_update = aknowledge.len() >= 10;
                        }
                        Update::KeyPackageCount(mut key_package_count) => {
                            send_update = key_package_count < WANTED_KEY_PACKAGES;
                            while key_package_count < WANTED_KEY_PACKAGES {
                                let key_package =
                                    client.create_key_package().await?.to_unverified();
                                key_packages.push(key_package);
                                key_package_count += 1;
                            }
                        }
                        Update::YieldRequest => {
                            send_update = true;
                            should_yield = true;
                        }
                    }
                } else {
                    // Timeout, so probably at the newest messages
                    //
                    // long next timeout, since everything is taken care of.
                    // Technically, there's not even a timeout neccesary.
                    timeout_duration = Duration::from_secs(60);

                    // Here we check if we want to add our session to any needed groups
                    for (device, _) in persistent_data.devices() {
                        let group = SvalinGroupId::DeviceGroup(device.clone());
                        if !client.is_member(&group, self.session_mls.me()).await? {
                            let key_package = self.session_mls.create_key_package().await?;
                            let message = client.add_member(&group, key_package).await?;
                            messages.push(message);
                        }

                        if let Some(message) = client
                            .create_meta_group_if_missing(device.clone())
                            .await
                            .map_err(|err| anyhow!(err))?
                        {
                            tracing::debug!("new meta group: {message:?}");
                            messages.push(message);
                        }
                        let meta_group = SvalinGroupId::DeviceMetaGroup(device.clone());
                        if !client.is_member(&meta_group, self.session_mls.me()).await? {
                            let key_package = self.session_mls.create_key_package().await?;
                            let message = client.add_member(&meta_group, key_package).await?;
                            messages.push(message);
                        }
                    }

                    // We likely just found a group which contains data we don't have yet.
                    // So it's a good idea to send that update to the session's state
                    if !messages.is_empty() {
                        self.state_handle
                            .update(ClientStateUpdate::Persistent(
                                persistent::Message::UpdateFromMainState(persistent_data.clone()),
                            ))
                            .await?;
                    }

                    send_update = !aknowledge.is_empty() || !messages.is_empty();
                }

                if send_update {
                    let mls_store = export_handle.export(&self.key)?;
                    let state_update = ToServer::StateUpdate {
                        mls_store,
                        persistent_data: EncryptedObject::encrypt(&persistent_data, &self.key)?,
                        aknowledged: mem::replace(&mut aknowledge, Vec::new()),
                        // TODO
                        key_packages: mem::replace(&mut key_packages, Vec::new()),
                        messages: mem::replace(&mut messages, Vec::new()),
                    };

                    session.write_object(&state_update).await?;
                } else {
                    session.write_object(&ToServer::OK).await?;
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
struct SavedState {
    mls_store: ExportedMlsStore,
    persistent_data: EncryptedObject<persistent::State>,
}

#[derive(Serialize, Deserialize, Debug)]
enum Update {
    Message(Uuid, Arc<MessageToMemberTransport>),
    KeyPackageCount(u64),
    YieldRequest,
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
    OK,
}
