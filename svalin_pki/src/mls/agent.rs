use crate::{Certificate, mls::client::MlsClient};

pub struct MlsAgent {
    mls: MlsClient,
}

pub enum MlsAgentCreateError {
    NotAnAgent(Certificate),
}

impl MlsAgent {
    pub fn new(
        credential: Credential,
        storage_provider: SqliteStorageProvider<PostcardCodec>,
    ) -> Self {
        // if credential.get_certificate() {}

        let mls = MlsClient::new(credential, storage_provider);
        Self { mls }
    }
}
