use openmls::prelude::ProtocolVersion;
use openmls_rust_crypto::RustCrypto;
use openmls_sqlx_storage::SqliteStorageProvider;
use openmls_traits::OpenMlsProvider;

#[derive(Default)]
pub struct PostcardCodec {}

impl openmls_sqlx_storage::Codec for PostcardCodec {
    type Error = postcard::Error;

    fn to_vec<T: serde::Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, Self::Error> {
        postcard::to_stdvec(value)
    }

    fn from_slice<T: serde::de::DeserializeOwned + ?Sized>(slice: &[u8]) -> Result<T, Self::Error> {
        postcard::from_bytes(slice)
    }
}

pub struct SvalinProvider {
    crypto: RustCrypto,
    storage_provider: SqliteStorageProvider<PostcardCodec>,
    protocol_version: ProtocolVersion,
}

impl SvalinProvider {
    pub fn new(storage_provider: SqliteStorageProvider<PostcardCodec>) -> Self {
        let crypto = RustCrypto::default();
        Self {
            crypto,
            storage_provider,
            protocol_version: ProtocolVersion::Mls10,
        }
    }

    pub fn protocol_version(&self) -> ProtocolVersion {
        self.protocol_version
    }
}

impl OpenMlsProvider for SvalinProvider {
    type CryptoProvider = RustCrypto;

    type RandProvider = RustCrypto;

    type StorageProvider = SqliteStorageProvider<PostcardCodec>;

    fn storage(&self) -> &Self::StorageProvider {
        &self.storage_provider
    }

    fn crypto(&self) -> &Self::CryptoProvider {
        &self.crypto
    }

    fn rand(&self) -> &Self::RandProvider {
        &self.crypto
    }
}
