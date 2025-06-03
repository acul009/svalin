use anyhow::{Context, Result, anyhow};

use ring::{agreement::X25519, signature::{Ed25519KeyPair, KeyPair, VerificationAlgorithm, ED25519}};
use rustls::pki_types::alg_id::{ECDSA_P521, ED25519};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct SignedMessage<'a> {
    message: &'a [u8],
    signature: &'a [u8],
}

pub(crate) trait CanVerify {
    fn borrow_public_key(&self) -> &[u8];
}

pub(crate) trait Verify {
    fn verify(&self, signed_message: &[u8]) -> Result<Vec<u8>>;
}

impl<T> Verify for T
where
    T: CanVerify,
{
    fn verify(&self, signed_message: &[u8]) -> Result<Vec<u8>> {
        let public_key = self.borrow_public_key();
        SignedMessage::verify(signed_message, public_key)
    }
}

pub(crate) trait CanSign {
    fn borrow_keypair(&self) -> &Ed25519KeyPair;
}

pub(crate) trait Sign {
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>>;
}

pub(crate) trait Encrypt {
    fn encrypt_for(&self, message: &[u8]) -> Result<Vec<u8>>;
}

impl<T> Sign for T
where
    T: CanSign,
{
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>> {
        let keypair = self.borrow_keypair();
        SignedMessage::create(message, keypair)
    }
}

impl<T> Encrypt for T
where
    T: CanSign,
{
    fn encrypt_for(&self, message: &[u8]) -> Result<Vec<u8>> {
        let keypair = self.borrow_keypair();
        SignedMessage::create(message, keypair)
    }
}

impl<T> Decrypt for T
where
    T: CanSign,
{
    fn decrypt(&self, message: &[u8]) -> Result<Vec<u8>> {
        let keypair = self.borrow_keypair();
    }
}

impl SignedMessage<'_> {
    pub fn create(message: &[u8], keypair: &Ed25519KeyPair) -> Result<Vec<u8>> {
        let signature = keypair.sign(message);

        let vec = SignedMessage::encode(message, signature.as_ref())?;

        Ok(vec)
    }

    pub fn verify(signed_message: &[u8], public_key: &[u8]) -> Result<Vec<u8>> {
        let (message, signature) = SignedMessage::decode(signed_message)?;
        ED25519
            .verify(public_key.into(), message.into(), signature.into())
            .map_err(|err| anyhow!(err))
            .context("signature verification failed")?;

        Ok(message.to_owned())
    }

    fn encode(message: &[u8], signature: &[u8]) -> Result<Vec<u8>> {
        let signed = SignedMessage { message, signature };

        let vec = postcard::to_extend(&signed, Vec::<u8>::new())?;

        Ok(vec)
    }

    fn decode(signed_message: &[u8]) -> Result<(&[u8], &[u8])> {
        let decoded: SignedMessage = postcard::from_bytes(signed_message)?;
        Ok((decoded.message, decoded.signature))
    }
}
