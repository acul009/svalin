use openmls::prelude::{
    Ciphersuite, CredentialWithKey, KeyPackage, KeyPackageBundle, KeyPackageNewError,
};
use openmls_rust_crypto::RustCrypto;
use openmls_sqlx_storage::SqliteStorageProvider;
use openmls_traits::OpenMlsProvider;

use crate::Credential;

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
}

impl SvalinProvider {
    pub fn new(storage_provider: SqliteStorageProvider<PostcardCodec>) -> Self {
        let crypto = RustCrypto::default();
        Self {
            crypto,
            storage_provider,
        }
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

pub struct MlsClient {
    provider: SvalinProvider,
    svalin_credential: Credential,
    mls_credential_with_key: CredentialWithKey,
}

impl MlsClient {
    pub fn new(
        credential: Credential,
        storage_provider: SqliteStorageProvider<PostcardCodec>,
    ) -> Self {
        let public_info = CredentialWithKey {
            credential: credential.get_certificate().into(),
            signature_key: credential.get_certificate().public_key().into(),
        };
        Self {
            provider: SvalinProvider::new(storage_provider),
            svalin_credential: credential,
            mls_credential_with_key: public_info,
        }
    }

    fn ciphersuite(&self) -> Ciphersuite {
        // ChaCha20 icompatible with rust crypto
        Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519
    }

    pub fn create_key_package(&self) -> Result<KeyPackageBundle, KeyPackageNewError> {
        KeyPackage::builder().build(
            self.ciphersuite(),
            &self.provider,
            &self.svalin_credential,
            self.mls_credential_with_key.clone(),
        )
    }
}
