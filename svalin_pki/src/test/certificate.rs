use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};

use crate::{
    Credential, KeyPair, certificate::UnverifiedCertificate, generate_key, get_current_timestamp,
    keypair::ExportedPublicKey,
};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct SerializationTestStruct {
    cert1: UnverifiedCertificate,
    cert2: UnverifiedCertificate,
}

#[test]
fn test_certificate_serde_serialization() {
    let credentials = Credential::generate_temporary().unwrap();
    let credentials2 = Credential::generate_temporary().unwrap();

    let test_struct = SerializationTestStruct {
        cert1: credentials.certificate().to_owned().to_unverified(),
        cert2: credentials2.certificate().to_owned().to_unverified(),
    };

    let encoded = rmp_serde::to_vec_named(&test_struct).unwrap();

    let rebuilt: SerializationTestStruct = rmp_serde::from_slice(&encoded).unwrap();

    assert_eq!(test_struct, rebuilt);
}

#[test]
pub fn serialization() {
    let perm_creds = Credential::generate_temporary().unwrap();
    let cert = perm_creds.certificate();

    let seriaized = cert.as_der().to_owned();
    let cert2 = UnverifiedCertificate::from_der(seriaized).unwrap();
    assert_eq!(cert, &cert2)
}

#[test]
pub fn serde_serialization() {
    let perm_creds = Credential::generate_temporary().unwrap();
    let cert = perm_creds.certificate().as_unverified();

    let serialized = rmp_serde::to_vec_named(&cert).unwrap();

    let cert2: UnverifiedCertificate = rmp_serde::from_slice(&serialized).unwrap();
    assert_eq!(cert, &cert2)
}

#[tokio::test]
async fn test_on_disk_storage() {
    let original = Credential::generate_temporary().unwrap();

    let rand = SystemRandom::new();

    let mut pw_seed = [0u8; 32];
    rand.fill(&mut pw_seed).unwrap();
    let key = generate_key().unwrap();

    let encrypted_credentials = original.export(&key).unwrap();

    let copy = encrypted_credentials.decrypt(&key).unwrap();

    assert_eq!(copy.certificate(), original.certificate());
}

#[tokio::test]
async fn test_create_leaf() {
    let root = Credential::generate_root().unwrap();

    let keypair = KeyPair::generate();

    let public_key = keypair.export_public_key();
    let serialized = rmp_serde::to_vec_named(&public_key).unwrap();

    let public_key: ExportedPublicKey = rmp_serde::from_slice(&serialized).unwrap();

    let leaf = root
        .create_agent_certificate_for_key(&public_key)
        .unwrap()
        .to_unverified();

    let serialized = rmp_serde::to_vec_named(&leaf).unwrap();
    let leaf: UnverifiedCertificate = rmp_serde::from_slice(&serialized).unwrap();

    let _verified = leaf
        .verify_signature(root.certificate(), get_current_timestamp())
        .unwrap();
}
