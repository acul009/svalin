use openmls_sqlx_storage::Codec;
use serde::{Deserialize, Serialize};

use crate::{EncryptedObject, generate_key, mls::provider::MessagePackCodec};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Original {
    hostname: String,
    online: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Extended {
    online: bool,
    quarantine_count: Option<u64>,
    hostname: String,
    #[serde(default)]
    labels: Vec<String>,
}

#[test]
fn named_fields_support_schema_evolution() {
    let original = Original {
        hostname: "gateway".into(),
        online: true,
    };
    let encoded = MessagePackCodec::to_vec(&original).unwrap();
    let extended: Extended = MessagePackCodec::from_slice(&encoded).unwrap();
    assert_eq!(extended.hostname, original.hostname);
    assert!(extended.online);
    assert_eq!(extended.quarantine_count, None);
    assert!(extended.labels.is_empty());

    let extended = Extended {
        quarantine_count: Some(12),
        labels: vec!["mail".into()],
        ..extended
    };
    let encoded = MessagePackCodec::to_vec(&extended).unwrap();
    let decoded: Original = MessagePackCodec::from_slice(&encoded).unwrap();
    assert_eq!(decoded, original);
}

#[test]
fn encrypted_payload_uses_named_fields() {
    let key = generate_key().unwrap();
    let original = Original {
        hostname: "gateway".into(),
        online: true,
    };
    let encrypted = EncryptedObject::encrypt(&original, &key).unwrap();
    let encoded = MessagePackCodec::to_vec(&encrypted).unwrap();
    let encrypted: EncryptedObject<Extended> = MessagePackCodec::from_slice(&encoded).unwrap();
    let decoded = encrypted.decrypt(&key).unwrap();
    assert_eq!(decoded.hostname, original.hostname);
    assert_eq!(decoded.quarantine_count, None);
    assert!(decoded.labels.is_empty());
}
