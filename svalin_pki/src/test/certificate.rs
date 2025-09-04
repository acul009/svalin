use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};

use crate::{
    Certificate, Credential, KeyPair,
    keypair::ExportedPublicKey,
    signed_message::{Sign, Verify},
};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
struct SerializationTestStruct {
    cert1: Certificate,
    cert2: Certificate,
}

#[test]
fn test_certificate_serde_serialization() {
    let credentials = Credential::generate_temporary().unwrap();
    let credentials2 = Credential::generate_temporary().unwrap();

    let test_struct = SerializationTestStruct {
        cert1: credentials.get_certificate().to_owned(),
        cert2: credentials2.get_certificate().to_owned(),
    };

    let encoded = postcard::to_extend(&test_struct, Vec::new()).unwrap();

    let rebuilt: SerializationTestStruct = postcard::from_bytes(&encoded).unwrap();

    assert_eq!(test_struct, rebuilt);
}

#[test]
pub fn cert_verify_message() {
    let credentials = Credential::generate_temporary().unwrap();
    let rand = SystemRandom::new();

    let mut msg = [0u8; 1024];
    rand.fill(&mut msg).unwrap();

    let signed = credentials.sign(&msg).unwrap();

    let msg2 = credentials.get_certificate().verify(&signed).unwrap();

    assert_eq!(msg, msg2.as_ref());
}

#[test]
pub fn serialization() {
    let perm_creds = Credential::generate_temporary().unwrap();
    let cert = perm_creds.get_certificate();

    let seriaized = cert.to_der().to_owned();
    let cert2 = Certificate::from_der(seriaized).unwrap();
    assert_eq!(cert, &cert2)
}

#[test]
pub fn serde_serialization() {
    let perm_creds = Credential::generate_temporary().unwrap();
    let cert = perm_creds.get_certificate();

    let serialized = postcard::to_extend(cert, Vec::new()).unwrap();

    let cert2: Certificate = postcard::from_bytes(&serialized).unwrap();
    assert_eq!(cert, &cert2)
}

#[tokio::test]
async fn test_on_disk_storage() {
    let original = Credential::generate_temporary().unwrap();

    let rand = SystemRandom::new();

    let mut pw_seed = [0u8; 32];
    rand.fill(&mut pw_seed).unwrap();
    let pw = String::from_utf8(
        pw_seed
            .iter()
            .map(|rand_num| (*rand_num & 0b00011111u8) + 58u8)
            .collect(),
    )
    .unwrap();

    let encrypted_credentials = original.export(pw.clone().into()).await.unwrap();

    let copy = encrypted_credentials.decrypt(pw.into()).await.unwrap();

    assert_eq!(copy.get_certificate(), original.get_certificate());
}

#[tokio::test]
async fn test_create_leaf() {
    let root = Credential::generate_temporary().unwrap();

    let keypair = KeyPair::generate();

    let public_key = keypair.export_public_key();
    let serialized = postcard::to_extend(&public_key, Vec::new()).unwrap();

    let public_key: ExportedPublicKey = postcard::from_bytes(&serialized).unwrap();

    let leaf = root.create_agent_certificate_for_key(&public_key).unwrap();

    let serialized = postcard::to_extend(&leaf, Vec::new()).unwrap();
    let leaf: Certificate = postcard::from_bytes(&serialized).unwrap();

    leaf.verify_signature(root.get_certificate()).unwrap()
}
