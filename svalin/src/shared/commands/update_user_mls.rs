use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::{Context, anyhow};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svalin_client_store::persistent;
use svalin_pki::{
    CertificateChainBuilder, CertificateType, Credential, EncryptedObject, ExactVerififier,
    RootCertificate, SpkiHash, Verifier, get_current_timestamp,
    mls::{
        client::{MessageData, MlsClient},
        key_package::UnverifiedKeyPackage,
        provider::{ExportedMlsStore, SvalinStorage},
        transport_types::MessageToMemberTransport,
    },
};
use svalin_rpc::rpc::command::{dispatcher::CommandDispatcher, handler::CommandHandler};
use svalin_server_store::{KeyPackageStore, MessageStore, UserStore};
use svalin_sysctl::sytem_report::SystemReport;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use uuid::Uuid;

use crate::{
    remote_key_retriever::RemoteKeyRetriever,
    server::MlsServer,
    verifier::{local_verifier::LocalVerifier, remote_verifier::RemoteVerifier},
};

pub struct UpdateUserMlsHandler {
    user_store: Arc<UserStore>,
    message_store: Arc<MessageStore>,
    key_package_store: Arc<KeyPackageStore>,
    verifier: LocalVerifier,
    mls: Arc<MlsServer>,
    user_lock: Mutex<HashMap<SpkiHash, Arc<tokio::sync::Mutex<()>>>>,
}

impl UpdateUserMlsHandler {
    pub fn new(
        verifier: LocalVerifier,
        user_store: Arc<UserStore>,
        message_store: Arc<MessageStore>,
        key_package_store: Arc<KeyPackageStore>,
        mls: Arc<MlsServer>,
    ) -> Self {
        Self {
            verifier,
            user_store,
            message_store,
            key_package_store,
            mls,
            user_lock: Mutex::new(HashMap::new()),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct UpdateData {
    mls_store: ExportedMlsStore,
    persistent_data: EncryptedObject<persistent::State>,
    messages: Vec<(Uuid, MessageToMemberTransport)>,
    key_package_count: u64,
}

#[derive(Serialize, Deserialize)]
struct UpdateResponse {
    mls_store: ExportedMlsStore,
    persistent_data: EncryptedObject<persistent::State>,
    handled: Vec<Uuid>,
    key_packages: Vec<UnverifiedKeyPackage>,
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
        _cancel: CancellationToken,
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

        let messages = self.message_store.load_all_for(&user_hash).await?;
        let current_packages = self
            .key_package_store
            .count_key_packages(&user_hash)
            .await?;

        let data = UpdateData {
            mls_store: user.mls_store,
            persistent_data: user.persistent_data,
            messages: messages,
            key_package_count: current_packages,
        };
        session.write_object(&data).await?;

        let response = session.read_object::<UpdateResponse>().await?;

        let user_cert = self
            .verifier
            .verify_spki_hash(&user_hash, get_current_timestamp())
            .await?;
        let verifier = ExactVerififier::new(user_cert);
        for key_package in response.key_packages {
            let key_package = self
                .mls
                .verify_key_package(key_package, &verifier)
                .await
                .context("error verifying key package")?;
            self.key_package_store.add_key_package(key_package).await?;
        }

        let new_count = self
            .key_package_store
            .count_key_packages(&user_hash)
            .await?;
        debug!("new key package count: {}", new_count);

        self.user_store
            .update_mls_data(&user_hash, response.mls_store, response.persistent_data)
            .await?;

        self.message_store
            .aknowledge_messages(&user_hash, &response.handled)
            .await?;

        session.write_object(&()).await?;

        // explicit drop, so I don't accidentally drop it beforehand
        drop(_user_lock);
        Ok(())
    }
}

pub struct UpdateUserMls {
    pub password: Vec<u8>,
    pub user_credential: Credential,
    pub key_retriever: RemoteKeyRetriever,
    pub verifier: RemoteVerifier,
}

impl CommandDispatcher for UpdateUserMls {
    type Output = persistent::State;

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
        let data = session.read_object::<UpdateData>().await?;

        let (mls_store, export_handle) =
            SvalinStorage::import(data.mls_store, self.password.clone()).await?;
        let mut persistent_data = data
            .persistent_data
            .decrypt_with_password(self.password.clone())
            .await?;
        let client = MlsClient::new(
            self.user_credential,
            mls_store,
            self.key_retriever,
            self.verifier,
        )?;
        debug!("Temporary MLS client created, processing messages");

        let mut handled = Vec::new();

        for (uuid, message) in data.messages {
            let processed = client
                .handle_message::<SystemReport>(message)
                .await
                .map_err(|err| anyhow!(err))?;
            handled.push(uuid);
            match processed {
                MessageData::Report(spki_hash, report) => {
                    persistent_data
                        .update(persistent::Message::UpdateSystemReport(spki_hash, report));
                }
                MessageData::Internal => (),
            }
        }

        let mut key_packages = Vec::new();
        while data.key_package_count + (key_packages.len() as u64) < 100 {
            let key_package = client.create_key_package().await?.to_unverified();
            key_packages.push(key_package);
        }

        let encrypted_data =
            EncryptedObject::encrypt_with_password(&persistent_data, self.password.clone()).await?;
        let exported_mls_store = export_handle.export(self.password.clone()).await?;

        debug!("MLS client processed messages, sending response");

        let response = UpdateResponse {
            mls_store: exported_mls_store,
            persistent_data: encrypted_data,
            handled,
            key_packages,
        };

        session.write_object(&response).await?;

        session.read_object::<()>().await?;

        Ok(persistent_data)
    }
}
