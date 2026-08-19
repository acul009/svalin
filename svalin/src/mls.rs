use serde::{Deserialize, Serialize};
use svalin_pki::TrustStoreVerifier;
use svalin_store::client_store::persistent::{SvalinMetaInfo, SvalinReport};

use crate::{
    remote_key_retriever::RemoteKeyRetriever, server::local_key_retriever::LocalKeyRetriever,
};

#[derive(Serialize, Deserialize)]
pub struct MlsTypes {}

impl svalin_pki::mls::transport_types::MessageTypes for MlsTypes {
    type Report = SvalinReport;

    type MetaInfo = SvalinMetaInfo;
}

pub type MlsClient =
    svalin_pki::mls::client::MlsClient<MlsTypes, RemoteKeyRetriever, TrustStoreVerifier>;
pub type MlsAgent =
    svalin_pki::mls::agent::MlsAgent<MlsTypes, RemoteKeyRetriever, TrustStoreVerifier>;
pub type MlsServer = svalin_pki::mls::server::MlsServer<LocalKeyRetriever, TrustStoreVerifier>;
