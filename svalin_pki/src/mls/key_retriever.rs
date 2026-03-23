use std::pin::Pin;

use crate::{
    SpkiHash,
    mls::key_package::{KeyPackage, UnverifiedKeyPackage},
};

pub trait KeyRetriever {
    type Error;

    fn get_required_device_group_members(
        &self,
        device: &SpkiHash,
    ) -> impl Future<Output = Result<Vec<SpkiHash>, Self::Error>>;
    fn get_key_packages(
        &self,
        entities: &[SpkiHash],
    ) -> impl Future<Output = Result<Vec<UnverifiedKeyPackage>, Self::Error>>;
}
