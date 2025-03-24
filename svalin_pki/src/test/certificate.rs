use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};

use crate::{
    Certificate, Keypair,
    signed_message::{Sign, Verify},
};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
struct SerializationTestStruct {
    cert1: Certificate,
    cert2: Certificate,
}

#[test]
fn test_certificate_serde_serialization() {
    let credentials = Keypair::generate().to_self_signed_cert().unwrap();
    let credentials2 = Keypair::generate().to_self_signed_cert().unwrap();

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
    let credentials = Keypair::generate();
    let rand = SystemRandom::new();

    let mut msg = [0u8; 1024];
    rand.fill(&mut msg).unwrap();

    let signed = credentials.sign(&msg).unwrap();

    let cert = credentials.to_self_signed_cert().unwrap();
    let msg2 = cert.verify(&signed).unwrap();
    let msg3 = cert.get_certificate().verify(&signed).unwrap();

    assert_eq!(msg, msg2.as_ref());
    assert_eq!(msg, msg3.as_ref());
}

#[test]
pub fn serialization() {
    let credentials = Keypair::generate();
    let perm_creds = credentials.to_self_signed_cert().unwrap();
    let cert = perm_creds.get_certificate();

    let seriaized = cert.to_der().to_owned();
    let cert2 = Certificate::from_der(seriaized).unwrap();
    assert_eq!(cert, &cert2)
}

#[test]
pub fn serde_serialization() {
    let credentials = Keypair::generate();
    let perm_creds = credentials.to_self_signed_cert().unwrap();
    let cert = perm_creds.get_certificate();

    let serialized = postcard::to_extend(cert, Vec::new()).unwrap();

    let cert2: Certificate = postcard::from_bytes(&serialized).unwrap();
    assert_eq!(cert, &cert2)
}
