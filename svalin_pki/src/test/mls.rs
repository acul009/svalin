use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    fmt::Display,
    sync::Arc,
};

use openmls::{
    group::GroupId,
    prelude::{MlsMessageBodyIn, MlsMessageIn, MlsMessageOut, ProtocolMessage, Welcome},
};
use openmls_sqlx_storage::SqliteStorageProvider;
use sqlx::SqlitePool;
use tls_codec::DeserializeBytes;

use crate::{
    Certificate, Credential, KeyPair, SpkiHash, Verifier, VerifyError,
    mls::{
        agent::MlsAgent,
        client::{MessageData, MlsClient},
        key_package::{KeyPackage, UnverifiedKeyPackage},
        key_retriever::{self, KeyRetriever},
        processor::{MlsProcessorHandle, ProcessedMessage},
        provider::PostcardCodec,
        public_processor::{self, PublicProcessorHandle},
        server::MlsServer,
        transport_types::{MessageToMember, MessageToServer},
    },
};

async fn create_processor(credential: Credential) -> MlsProcessorHandle {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let storage = SqliteStorageProvider::<PostcardCodec>::new(pool);
    storage.run_migrations().await.unwrap();
    let handle = crate::mls::processor::MlsProcessorHandle::new_processor(credential, storage);
    handle
}

async fn create_public_processor() -> PublicProcessorHandle {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let storage = SqliteStorageProvider::<PostcardCodec>::new(pool);
    storage.run_migrations().await.unwrap();
    let handle = PublicProcessorHandle::new(storage);
    handle
}

#[tokio::test]
async fn test_key_package_creation() {
    let credential = Credential::generate_root().unwrap();
    let processor = create_processor(credential).await;
    let _key_package = processor.create_key_package().await.unwrap();
}

#[tokio::test]
async fn test_processor() {
    let credential1 = Credential::generate_root().unwrap();
    let member1 = create_processor(credential1).await;

    let credential2 = Credential::generate_root().unwrap();
    let member2 = create_processor(credential2).await;
    let key_package = member2.create_key_package().await.unwrap();

    let group_id = GroupId::from_slice(b"test");

    let MessageToMember::Welcome(welcome) = member1
        .create_group(vec![key_package], group_id.clone())
        .await
        .unwrap()
        .to_member()
        .unwrap()
    else {
        panic!("wrong message type")
    };

    let staged = member2.stage_join(welcome).await.unwrap();
    member2.join_group(staged).await.unwrap();

    let test_text = b"Hello MLS!".to_vec();

    let message = member2
        .create_message(group_id, test_text.clone())
        .await
        .unwrap();

    let MessageToMember::GroupMessage(message) = message.to_member().unwrap() else {
        panic!("wrong message type")
    };

    let received = member1.process_message(message).await.unwrap();

    assert_eq!(test_text, received.decrypted);
}

#[tokio::test]
async fn test_public_processor() {
    let credential1 = Credential::generate_root().unwrap();
    let spki_hash1 = credential1.certificate().spki_hash().clone();
    let member1 = create_processor(credential1).await;

    let credential2 = Credential::generate_root().unwrap();
    let spki_hash2 = credential2.certificate().spki_hash().clone();
    let member2 = create_processor(credential2).await;
    let key_package = member2.create_key_package().await.unwrap();

    let group_id = GroupId::from_slice(b"test");

    let new_group = member1
        .create_group(vec![key_package], group_id.clone())
        .await
        .unwrap()
        .unpack()
        .unwrap();

    let public_processor = create_public_processor().await;

    let to_send = public_processor.add_group(new_group).await.unwrap();
    let members = to_send
        .receivers
        .into_iter()
        .filter(|spki_hash| spki_hash != &spki_hash1)
        .collect::<Vec<_>>();
    let MessageToMember::Welcome(welcome) = to_send.message.unpack().unwrap() else {
        panic!("wrong message type")
    };

    assert_eq!(members.len(), 1);
    assert_eq!(members[0], spki_hash2);

    let staged = member2.stage_join(welcome).await.unwrap();
    member2.join_group(staged).await.unwrap();

    let test_text = b"Hello MLS!".to_vec();

    #[allow(irrefutable_let_patterns)]
    let MessageToServer::GroupMessage(message) = member2
        .create_message(group_id, test_text.clone())
        .await
        .unwrap()
    else {
        panic!("wrong message type")
    };

    let to_send = public_processor.process_message(message).await.unwrap();

    let members = to_send
        .receivers
        .into_iter()
        .filter(|spki_hash| spki_hash != &spki_hash2)
        .collect::<Vec<_>>();

    assert_eq!(members.len(), 1);
    assert_eq!(members[0], spki_hash1);

    let MessageToMember::GroupMessage(message) = to_send.message.unpack().unwrap() else {
        panic!("wrong message type")
    };

    let received = member1.process_message(message).await.unwrap();

    assert_eq!(test_text, received.decrypted);
}

#[derive(Clone)]
struct TestRetriever {
    key_packages: Arc<RefCell<HashMap<SpkiHash, UnverifiedKeyPackage>>>,
}

impl TestRetriever {
    fn new() -> Self {
        Self {
            key_packages: Arc::new(RefCell::new(HashMap::new())),
        }
    }

    fn add(&self, key_package: KeyPackage) {
        self.key_packages
            .borrow_mut()
            .insert(key_package.spki_hash().clone(), key_package.to_unverified());
    }
}

#[derive(Debug)]
enum Never {}

impl Display for Never {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unreachable!()
    }
}

impl KeyRetriever for TestRetriever {
    type Error = Never;

    async fn get_required_group_members(
        &self,
        _id: &crate::mls::SvalinGroupId,
    ) -> Result<Vec<crate::SpkiHash>, Self::Error> {
        Ok(self.key_packages.borrow().keys().cloned().collect())
    }

    async fn get_key_packages(
        &self,
        entities: &[crate::SpkiHash],
    ) -> Result<Vec<crate::mls::key_package::UnverifiedKeyPackage>, Self::Error> {
        let key_packages = self.key_packages.borrow();
        Ok(entities
            .iter()
            .map(|hash| key_packages.get(hash).unwrap().clone())
            .collect())
    }
}

#[derive(Debug, Clone)]
struct TestVerifier {
    known: HashMap<SpkiHash, Certificate>,
}

impl TestVerifier {
    fn new() -> Self {
        Self {
            known: HashMap::new(),
        }
    }

    fn push(&mut self, cert: Certificate) {
        self.known.insert(cert.spki_hash().clone(), cert);
    }
}

impl Verifier for TestVerifier {
    async fn verify_spki_hash(
        &self,
        spki_hash: &SpkiHash,
        time: u64,
    ) -> Result<Certificate, crate::VerifyError> {
        let cert = self
            .known
            .get(spki_hash)
            .ok_or(VerifyError::UnknownCertificate)?;

        cert.check_validity_at(time)?;

        Ok(cert.clone())
    }
}

#[tokio::test]
async fn test_device_group() {
    let mut verifier = TestVerifier::new();
    let retriever = TestRetriever::new();

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let client_storage = SqliteStorageProvider::<PostcardCodec>::new(pool);
    client_storage.run_migrations().await.unwrap();
    let user_credential = Credential::generate_root().unwrap();
    verifier.push(user_credential.certificate().clone());

    let client_credential = user_credential.create_user_device_credential().unwrap();
    verifier.push(client_credential.certificate().clone());

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let agent_storage = SqliteStorageProvider::<PostcardCodec>::new(pool.clone());
    agent_storage.run_migrations().await.unwrap();
    let keypair = KeyPair::generate();
    let public_key = keypair.export_public_key();
    let cert = user_credential
        .create_agent_certificate_for_key(&public_key)
        .unwrap();
    let agent_credential = keypair.upgrade(cert.to_unverified()).unwrap();
    verifier.push(agent_credential.certificate().clone());

    let client = MlsClient::new(
        client_credential.clone(),
        client_storage,
        retriever.clone(),
        verifier.clone(),
    )
    .unwrap();
    retriever.add(client.create_key_package().await.unwrap());

    let agent = MlsAgent::new(
        agent_credential.clone(),
        agent_storage,
        retriever.clone(),
        verifier.clone(),
    )
    .await
    .unwrap();

    let agent_key_package = agent.create_key_package().await.unwrap().to_unverified();
    // Not actually used, just needed to the retriever shows the agent as needed
    retriever.add(agent.create_key_package().await.unwrap());

    let new_group = client
        .create_device_group(agent_credential.certificate().clone(), agent_key_package)
        .await
        .unwrap();

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let storage = SqliteStorageProvider::<PostcardCodec>::new(pool);
    storage.run_migrations().await.unwrap();
    let server = MlsServer::new(storage, verifier.clone(), retriever.clone());
    let welcome = server
        .add_device_group(new_group, agent_credential.certificate().spki_hash())
        .await
        .unwrap();

    let to_member = welcome.message.unpack().unwrap();

    agent.handle_message(to_member).await.unwrap();

    let report = "Test Data".to_string();
    let to_server = agent.send_report(report.clone()).await.unwrap();

    let to_send = server.process_message(to_server).await.unwrap();

    let received: MessageData<String> = client
        .handle_message(to_send.message.unpack().unwrap())
        .await
        .unwrap();

    let MessageData::Report(sender, received_report) = received else {
        panic!("wrong message type")
    };

    assert_eq!(&sender, agent_credential.certificate().spki_hash());
    assert_eq!(&received_report, &report);
}
