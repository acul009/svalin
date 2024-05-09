use std::fmt::Debug;

use anyhow::{anyhow, Context, Result};

use ring::signature::{Ed25519KeyPair, KeyPair};
use serde::{Deserialize, Serialize};

use crate::{
    encrypt::EncryptedData,
    signed_message::{CanSign, CanVerify},
    Certificate, CertificateRequest,
};

pub struct PermCredentials {
    keypair: Ed25519KeyPair,
    raw_keypair: Vec<u8>,
    certificate: Certificate,
}

impl Debug for PermCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PermCredentials")
            .field("certificate", &self.certificate)
            .finish()
    }
}

#[derive(Serialize, Deserialize)]
struct CredentialOnDisk {
    encrypted_keypair: Vec<u8>,
    raw_cert: Vec<u8>,
}

impl PermCredentials {
    pub(crate) fn new(raw_keypair: Vec<u8>, certificate: Certificate) -> Result<Self> {
        // TODO: check if keypair and certificate belong together

        // println!("{:?}", keypair.public_key().as_ref());
        // println!("{:?}", certificate.borrow_public_key());

        // if *keypair.public_key().as_ref() != *certificate.borrow_public_key() {
        //     bail!(crate::Error::KeyMismatch)
        // }

        let keypair = Ed25519KeyPair::from_pkcs8(&raw_keypair)
            .map_err(|err| anyhow!(err))
            .context("failed to decode keypair")?;

        Ok(PermCredentials {
            keypair,
            raw_keypair,
            certificate,
        })
    }

    pub fn to_bytes(&self, password: &[u8]) -> Result<Vec<u8>> {
        let encrypted_keypair = EncryptedData::encrypt_with_password(&self.raw_keypair, password)
            .context("Failed to encrypt keypair")?;
        let on_disk = CredentialOnDisk {
            encrypted_keypair,
            raw_cert: self.certificate.to_der().to_owned(),
        };

        let encoded = postcard::to_extend(&on_disk, Vec::new())?;

        Ok(encoded)
    }

    pub fn from_bytes(bytes: &[u8], password: &[u8]) -> Result<Self> {
        let on_disk: CredentialOnDisk =
            postcard::from_bytes(bytes).context("failed to decode postcard")?;

        let certificate =
            Certificate::from_der(on_disk.raw_cert).context("failed to decode certificate")?;

        let decrypted_keypair =
            EncryptedData::decrypt_with_password(&on_disk.encrypted_keypair, password)
                .context("failed to decrypt keypair")?;

        Self::new(decrypted_keypair, certificate)
    }

    pub fn get_certificate(&self) -> &Certificate {
        &self.certificate
    }

    pub fn get_key_bytes(&self) -> &[u8] {
        &self.raw_keypair
    }

    pub fn approve_request(&self, request: CertificateRequest) -> Result<Certificate> {
        let ca_keypair = rcgen::KeyPair::from_der(&self.raw_keypair)?;
        let ca_params =
            rcgen::CertificateParams::from_ca_cert_der(self.certificate.to_der(), ca_keypair)?;

        let ca = rcgen::Certificate::from_params(ca_params)?;

        let new_cert_der = request.csr.serialize_der_with_signer(&ca)?;

        let new_cert = Certificate::from_der(new_cert_der)?;

        Ok(new_cert)
    }
}

impl CanSign for PermCredentials {
    fn borrow_keypair(&self) -> &Ed25519KeyPair {
        &self.keypair
    }
}

impl CanVerify for PermCredentials {
    fn borrow_public_key(&self) -> &[u8] {
        self.keypair.public_key().as_ref()
    }
}

#[cfg(test)]
mod test {
    use ring::rand::{SecureRandom, SystemRandom};

    use crate::{Keypair, PermCredentials};

    #[test]
    fn test_on_disk_storage() {
        let original = Keypair::generate().unwrap().to_self_signed_cert().unwrap();

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

        let on_disk = original.to_bytes(pw.as_ref()).unwrap();

        let copy = PermCredentials::from_bytes(&on_disk, pw.as_ref()).unwrap();

        assert_eq!(copy.raw_keypair, original.raw_keypair);
        assert_eq!(copy.certificate, original.certificate);
    }
}
