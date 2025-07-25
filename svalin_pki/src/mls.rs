use openmls::prelude::CredentialType;
use openmls_traits::signatures::SignerError;

use crate::{Certificate, Credential, signed_message::CanSign};

impl From<&Certificate> for openmls::credentials::Credential {
    fn from(value: &Certificate) -> Self {
        // While there is the X509 credential type, it is not yet supported my openmls.
        // For now we'll have to use basic and handle verification ourselfves
        Self::new(CredentialType::Basic, value.to_der().to_vec())
    }
}

impl openmls_traits::signatures::Signer for Credential {
    fn sign(&self, payload: &[u8]) -> Result<Vec<u8>, SignerError> {
        Ok(self.borrow_keypair().sign(payload).as_ref().to_vec())
    }

    fn signature_scheme(&self) -> openmls::prelude::SignatureScheme {
        openmls::prelude::SignatureScheme::ED25519
    }
}
