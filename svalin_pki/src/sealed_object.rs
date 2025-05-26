use std::{collections::HashMap, marker::PhantomData};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
/// A sealed object will hybrid encrypt and sign a serializeable type.
/// When creating a sealed object, you'll have to provide the certificates which are allowed to decrypt the data.
/// To access the data again, you'll have to provide both some correct credentials as well as a verifier.
pub struct SealedObject<T> {
    phantom: PhantomData<T>,
    /// Key: spki_hash, Value: encrypted main key
    receiver_keys: HashMap<String, [u8; 32]>,
    signed_data: Vec<u8>,
}
