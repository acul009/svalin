use crate::{Credential, KeyPair, trust_store::TrustStore};

#[test]
fn test_trust_store() {
    let root_credential = Credential::generate_root().unwrap();
    let root = root_credential
        .certificate()
        .clone()
        .to_unverified()
        .use_as_root()
        .unwrap();

    let mut root_store = TrustStore::initialize(root.clone());

    let agent1 = KeyPair::generate();
    let cert = root_credential
        .create_agent_certificate_for_key(&agent1.export_public_key())
        .unwrap();
    let block = root_store.add(cert.clone(), &root_credential).unwrap();
    root_store.apply(block);
    let exported = root_store.export();

    let agent1 = agent1.upgrade(cert.to_unverified()).unwrap();
    let mut agent1_store = TrustStore::import(exported).unwrap();

    let agent2 = KeyPair::generate();
    let cert = root_credential
        .create_agent_certificate_for_key(&agent2.export_public_key())
        .unwrap();
    let agent2 = agent2.upgrade(cert.clone().to_unverified()).unwrap();

    // testing that unallowed transactions cannot be created
    agent1_store
        .add(agent2.certificate().clone(), &agent1)
        .unwrap_err();

    let block = root_store.add(cert.clone(), &root_credential).unwrap();
    let unchecked = block.as_unchecked().clone();
    root_store.apply(block);
    let exported = root_store.export();

    let block = agent1_store.check(unchecked).unwrap();
    agent1_store.apply(block);

    let _agent2_store = TrustStore::import(exported.clone()).unwrap();
}
