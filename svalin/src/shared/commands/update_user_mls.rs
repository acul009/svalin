use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::anyhow;
use async_trait::async_trait;
use svalin_client_store::persistent;
use svalin_pki::{
    CertificateType, Credential, EncryptedObject, ExactVerififier, SpkiHash,
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
use uuid::Uuid;

use crate::{
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
        _cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let peer = session.peer().certificate()?.clone();
        if peer.certificate_type() != CertificateType::UserDevice {
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

        session
            .write_object(&(user.mls_store, user.persistent_data))
            .await?;

        // Ok, so at this point I need a way to load all messages which should be delivered in order.
        // Once all of them are sent over, the dispatcher should get a signal that we're done, so we'll porbably be sending Options.
        // The Dispatcher should then send an updated version of the user's MlsState to the server.
        // I might want to somehow verify that new MlsState too. Maybe with a SignedObject?
        // Either way, the dispatcher should send both the updated MlsState, but also the ids of all messages which were processed.

        let messages = self.message_store.load_all_for(&user_hash).await?;

        for message in messages {
            session.write_object(&Some(message)).await?;
        }
        session
            .write_object(&None::<(Uuid, MessageToMemberTransport)>)
            .await?;

        let verifier = ExactVerififier::new(peer.clone());
        while let Some(key_package) = session
            .read_object::<Option<UnverifiedKeyPackage>>()
            .await?
        {
            let key_package = self.mls.verify_key_package(key_package, &verifier).await?;
            self.key_package_store.add_key_package(key_package).await?;
        }

        let current_packages = self
            .key_package_store
            .count_key_packages(&user_hash)
            .await?;
        session.write_object(&current_packages).await?;

        let (mls_store, persistent_data) = session
            .read_object::<(ExportedMlsStore, EncryptedObject<persistent::ClientState>)>()
            .await?;

        self.user_store
            .update_mls_data(&user_hash, mls_store, persistent_data)
            .await?;

        let handled = session.read_object::<Vec<Uuid>>().await?;

        self.message_store
            .aknowledge_messages(&user_hash, &handled)
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
    type Output = persistent::ClientState;

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
        let (mls_store, persistent_data) = session
            .read_object::<(ExportedMlsStore, EncryptedObject<persistent::ClientState>)>()
            .await?;
        let (mls_store, export_handle) =
            SvalinStorage::import(mls_store, self.password.clone()).await?;
        let mut persistent_data = persistent_data
            .decrypt_with_password(self.password.clone())
            .await?;
        let client = MlsClient::new(
            self.user_credential,
            mls_store,
            self.key_retriever,
            self.verifier,
        )?;

        let mut handled = Vec::new();

        while let Some((uuid, message)) = session
            .read_object::<Option<(Uuid, MessageToMemberTransport)>>()
            .await?
        {
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

        let mut current_packages = session.read_object::<u64>().await?;
        while current_packages < 100 {
            let key_package = Some(client.create_key_package().await?.to_unverified());
            session.write_object(&key_package).await?;
            current_packages += 1;
        }
        session.write_object(&None::<UnverifiedKeyPackage>).await?;

        let encrypted_data =
            EncryptedObject::encrypt_with_password(&persistent_data, self.password.clone()).await?;
        let exported_mls_store = export_handle.export(self.password.clone()).await?;

        session
            .write_object(&(exported_mls_store, encrypted_data))
            .await?;

        session.write_object(&handled).await?;

        session.read_object::<()>().await?;

        Ok(persistent_data)
    }
}
