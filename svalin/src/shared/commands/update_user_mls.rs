use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use anyhow::anyhow;
use async_trait::async_trait;
use svalin_client_store::persistent;
use svalin_pki::{
    CertificateType, EncryptedObject, SpkiHash,
    mls::{
        client::MlsClient, provider::{ExportedMlsStore, SvalinStorage}, transport_types::MessageToMemberTransport
    },
};
use svalin_rpc::rpc::command::{dispatcher::CommandDispatcher, handler::CommandHandler};
use svalin_server_store::{MessageStore, UserStore};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub struct UpdateUserMlsHandler {
    user_store: Arc<UserStore>,
    message_store: Arc<MessageStore>,
    user_lock: Mutex<HashMap<SpkiHash, Arc<tokio::sync::Mutex<()>>>>,
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
        request: Self::Request,
        cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let peer = session.peer().certificate()?;
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

        let messages = self.message_store.load_all_for(user_hash).await?;

        for message in messages {
            session.write_object(&Some(message)).await?;
        }
        session
            .write_object(&Option::<(Uuid, MessageToMemberTransport)>::None)
            .await?;

        todo!("aknowledge messages and save new states");

        // explicit drop, so I don't accidentally drop it beforehand
        drop(_user_lock);
        Ok(())
    }
}

pub struct UpdateUserMls {
    password: Vec<u8>,
}

impl CommandDispatcher for UpdateUserMls {
    type Output = persistent::ClientState;

    type Error = anyhow::Error;

    type Request = ();

    fn key() -> String {
        UpdateUserMlsHandler::key()
    }

    fn get_request(&self) -> &Self::Request {
        todo!()
    }

    async fn dispatch(
        self,
        session: &mut svalin_rpc::rpc::session::Session,
    ) -> Result<Self::Output, Self::Error> {
        let (mls_store, persistent_data) = session
            .read_object::<(ExportedMlsStore, EncryptedObject<persistent::ClientState>)>()
            .await?;
        let mls_store = SvalinStorage::import(mls_store, self.password.clone()).await?;
        let persistent_data = persistent_data.decrypt_with_password(self.password).await?;
        let client = MlsClient::new(credential, storage_provider, key_retriever, verifier)

        while let Some((uuid, message)) = session
            .read_object::<Option<(Uuid, MessageToMemberTransport)>>()
            .await?
        {
            todo!()
        }

        todo!()
    }
}
