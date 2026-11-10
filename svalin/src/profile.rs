use std::sync::{Arc, RwLock};

pub mod persistent;
pub mod warnings;

pub enum Update {
    TrustStore(svalin_pki::secure_chain::UncheckedBlock<svalin_pki::trust_store::Transaction>),
    Persistent(persistent::Update),
}

use anyhow::{Context, Result};
use svalin_pki::Credential;
use svalin_pki::trust_store::TrustStore;

use crate::client::Profile;
use crate::store::client_store::ClientStore;
use crate::util::location::{Location, LocationError};
use crate::util::trust_store::read_trust_store;

pub struct ProfileState {
    connection_info: ConnectionInfo,
    client_store: ClientStore,
    trust_store: Arc<RwLock<TrustStore>>,
    persistent: persistent::State,
    warnings: warnings::State,
}

impl ProfileState {
    pub fn update(&mut self, update: Update) -> Result<()> {
        match update {
            Update::TrustStore(block) => {
                let mut trust_store = self.trust_store.write().unwrap();
                let block = trust_store.check(block)?;
                trust_store.apply(block);
                let devices = self.persistent.update_from_trust_store(
                    &trust_store,
                    self.connection_info.device_credential.certificate().issuer(),
                );
                for device in devices {
                    self.warnings.update_device_warnings(&self.persistent, &device);
                }
            }
            Update::Persistent(update) => {
                let device = self.persistent.update(update);
                self.warnings.update_device_warnings(&self.persistent, &device);
            }
        }
        Ok(())
    }
}

pub enum ConnectionState {
    Connecting,
    Connected,
    Disconnected,
}

pub struct ConnectionInfo {
    device_credential: Credential,
    upstream_address: String,
}

pub async fn unlock(profile_name: &str, password: Vec<u8>) -> Result<ProfileState> {
    let profile = get_profile(profile_name)
        .await?
        .context("Profile not found")?;

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
    let trust_store =
        read_trust_store(&trust_store_path, client_store.transaction_store().as_ref()).await?;

    let mut persistent = persistent::State::default();
    let devices = persistent.update_from_trust_store(
        &trust_store,
        connection_info.device_credential.certificate().issuer(),
    );

    let mut warnings = warnings::State::new();
    for device in devices {
        warnings.update_device_warnings(&persistent, &device);
    }

    Ok(ProfileState {
        connection_info,
        client_store,
        trust_store: Arc::new(RwLock::new(trust_store)),
        persistent,
        warnings,
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
