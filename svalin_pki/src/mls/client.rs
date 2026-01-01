use openmls::prelude::ProtocolVersion;
use openmls_rust_crypto::RustCrypto;
use openmls_sqlite_storage::SqliteStorageProvider;
use openmls_traits::OpenMlsProvider;

#[derive(Debug)]
pub struct SvalinProvider {
    crypto: RustCrypto,
    key_store: SqliteStorageProvider<>,
    protocol_version: ProtocolVersion,
}

impl OpenMlsProvider for SvalinProvider {
    type CryptoProvider = RustCrypto;

    type RandProvider = RustCrypto;

    type StorageProvider = MemoryStorage;

    fn storage(&self) -> &Self::StorageProvider {
        &self.key_store
    }

    fn crypto(&self) -> &Self::CryptoProvider {
        &self.crypto
    }

    fn rand(&self) -> &Self::RandProvider {
        &self.crypto
    }
}

pub struct MlsClient {}
