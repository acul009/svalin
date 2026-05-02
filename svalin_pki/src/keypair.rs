use std::fmt::Debug;

use crate::{
    CreateCredentialsError, Credential, EncryptError, UnverifiedCertificate,
    encrypt::{EncryptedObject, EncryptionKey},
};
use anyhow::Result;
use rcgen::{PublicKeyData, SignatureAlgorithm};
use ring::signature::Ed25519KeyPair;
use serde::{Deserialize, Serialize};
use x509_parser::nom::AsBytes;
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Debug, thiserror::Error)]
pub enum DecodeKeypairError {
    #[error("error decrypting credentials: {0}")]
    DecryptError(#[from] crate::encrypt::DecryptError),
    #[error("error decoding credentials: {0}")]
    DecodeError(#[from] postcard::Error),
    #[error("error detecting key encoding")]
    DetectEncodingError,
    #[error("error parsing keypair: {0}")]
    ParseKeypairError(rcgen::Error),
}

pub struct KeyPair {
    keypair: rcgen::KeyPair,
    sign_keypair: Ed25519KeyPair,
}

impl Debug for KeyPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keypair").finish()
    }
}

impl KeyPair {
    pub fn generate() -> Self {
        let keypair = rcgen::KeyPair::generate_for(&rcgen::PKCS_ED25519).unwrap();
        let sign_keypair = Ed25519KeyPair::from_pkcs8(keypair.serialized_der()).unwrap();

        Self {
            keypair,
            sign_keypair,
        }
    }

    pub fn export_public_key(&self) -> ExportedPublicKey {
        ExportedPublicKey {
            der: self.keypair.der_bytes().to_vec(),
            alg: Algorithm::from_rcgen(self.keypair.algorithm()),
        }
    }

    pub fn upgrade(
        self,
        certificate: UnverifiedCertificate,
    ) -> Result<Credential, CreateCredentialsError> {
        Credential::new(self, certificate)
    }

    pub(crate) fn encrypt(
        &self,
        encryption_key: &EncryptionKey,
    ) -> Result<EncryptedObject<SavedKeypair>, EncryptError> {
        let saved = SavedKeypair {
            der: self.keypair.serialize_der(),
            alg: Algorithm::from_rcgen(self.keypair.algorithm()),
        };

        EncryptedObject::encrypt(&saved, encryption_key)
    }

    pub(crate) fn decrypt(
        ciphertext: EncryptedObject<SavedKeypair>,
        encryption_key: &EncryptionKey,
    ) -> Result<Self, DecodeKeypairError> {
        let saved = ciphertext.decrypt(encryption_key)?;

        let keypair = rcgen::KeyPair::from_der_and_sign_algo(
            &saved
                .der
                .as_bytes()
                .try_into()
                .map_err(|_err| DecodeKeypairError::DetectEncodingError)?,
            saved.alg.as_rcgen(),
        )
        .map_err(DecodeKeypairError::ParseKeypairError)?;

        let sign_keypair = Ed25519KeyPair::from_pkcs8(keypair.serialized_der()).unwrap();

        Ok(Self {
            keypair,
            sign_keypair,
        })
    }

    pub(crate) fn rcgen(&self) -> &rcgen::KeyPair {
        &self.keypair
    }

    pub(crate) fn rcgen_clone(&self) -> rcgen::KeyPair {
        let der = self.keypair.serialized_der().try_into().unwrap();
        rcgen::KeyPair::from_der_and_sign_algo(&der, self.keypair.algorithm())
            .expect("current value is valid, so new one should be too")
    }

    pub(crate) fn signing_keypair(&self) -> &Ed25519KeyPair {
        &self.sign_keypair
    }

    pub fn rustls_private_key(&self) -> rustls::pki_types::PrivateKeyDer<'static> {
        rustls::pki_types::PrivateKeyDer::try_from(self.keypair.serialized_der().to_owned())
            .unwrap()
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, Zeroize, PartialEq)]
enum Algorithm {
    #[default]
    PkcsEd25519,
}

impl Algorithm {
    pub fn as_rcgen(&self) -> &'static SignatureAlgorithm {
        match self {
            Algorithm::PkcsEd25519 => &rcgen::PKCS_ED25519,
        }
    }

    pub fn from_rcgen(alg: &'static SignatureAlgorithm) -> Self {
        if &rcgen::PKCS_ED25519 == alg {
            Self::PkcsEd25519
        } else {
            panic!("Algorithm not supported")
        }
    }
}

#[derive(Serialize, Deserialize, ZeroizeOnDrop)]
pub(crate) struct SavedKeypair {
    der: Vec<u8>,
    alg: Algorithm,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ExportedPublicKey {
    der: Vec<u8>,
    alg: Algorithm,
}

impl PublicKeyData for ExportedPublicKey {
    fn der_bytes(&self) -> &[u8] {
        &self.der
    }

    fn algorithm(&self) -> &'static SignatureAlgorithm {
        self.alg.as_rcgen()
    }
}

#[cfg(test)]
mod test {

    use crate::{KeyPair, generate_key};

    #[tokio::test]
    async fn test_encode_decode() {
        let credentials = KeyPair::generate();
        let key = generate_key().unwrap();

        let encrypted = credentials.encrypt(&key).unwrap();
        let _credentials2 = KeyPair::decrypt(encrypted, &key).unwrap();
    }
}
