
use std::sync::{Arc, RwLock};

pub mod persistent;
pub mod warnings;

pub enum Update {
    TrustStore(svalin_pki::secure_chain::UncheckedBlock<svalin_pki::trust_store::Transaction>),
    Persistent(persistent::Update),
}

use anyhow::{Context, Result};
use svalin_pki::trust_store::TrustStore;
use svalin_pki::Credential;

use crate::client::Profile;
use svalin_pki::SpkiHash;
use crate::store::client_store::ClientStore;
use crate::util::location::{Location, LocationError};
use crate::util::trust_store::read_trust_store;

pub struct ProfileState {
    connection_info: ConnectionInfo,
    client_store: ClientStore,
    trust_store: Arc<RwLock<TrustStore>>,
    persistent: persistent::State,
}

impl ProfileState {
    pub fn update_warnings(&mut self, devices: impl Iterator<Item = SpkiHash>) {
        todo!()
    }

    pub fn update(&mut self, update: Update) {
        todo!()
    }
}

pub struct ConnectionInfo {
    device_credential: Credential,
    upstream_address: String,
}

pub async fn unlock(profile_name: &str, password: Vec<u8>) -> Result<ProfileState> {
    let profile = get_profile(profile_name).await?.context("Profile not found")?;

    let key = profile
        .local_credential_params
        .derive_encryption_key(password)
        .await?;
    let device_credential = profile.device_credential.decrypt(&key)?;

    let connection_info = ConnectionInfo {
        device_credential,
        upstream_address: profile.upstream_address,
    };

    let profile_dir = profile_dir(profile_name).await?;
    let client_db_path = profile_dir.clone().push("client-store.sqlite");
    let client_store = ClientStore::open(client_db_path).await?;

    let trust_store_path = profile_dir.push("trust_store.json").to_pathbuf();
    let trust_store = read_trust_store(
        &trust_store_path,
        client_store.transaction_store().as_ref(),
    )
    .await?;

    let mut persistent = persistent::State::default();
    persistent.update_from_trust_store(
        &trust_store,
        connection_info.device_credential.certificate().issuer(),
    );

    Ok(ProfileState {
        connection_info,
        client_store,
        trust_store: Arc::new(RwLock::new(trust_store)),
        persistent,
    })
}

async fn data_dir() -> Result<Location, LocationError> {
    Location::user_data_dir()?
        .push("client")
        .ensure_parent_exists()
        .await
}

async fn profile_dir(profile_name: &str) -> Result<Location> {
    Ok(data_dir().await?.push(profile_name.replace(":", "+")))
}

async fn get_profile(profile_name: &str) -> Result<Option<Profile>> {
    let location = profile_dir(profile_name).await?.push("profile.json");

    if tokio::fs::try_exists(&location).await? {
        let json = tokio::fs::read_to_string(location).await?;
        let profile = serde_json::from_str(&json)?;

        Ok(Some(profile))
    } else {
        Ok(None)
    }
}
