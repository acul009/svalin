use std::{fmt::Display, pin::Pin};

use crate::{
    SpkiHash,
    mls::{
        group_id::SvalinGroupId,
        key_package::{KeyPackage, UnverifiedKeyPackage},
    },
};

pub trait KeyRetriever {
    type Error: Display;

    fn get_required_group_members(
        &self,
        id: &SvalinGroupId,
    ) -> impl Future<Output = Result<Vec<SpkiHash>, Self::Error>>;
    fn get_key_packages(
        &self,
        entities: &[SpkiHash],
    ) -> impl Future<Output = Result<Vec<UnverifiedKeyPackage>, Self::Error>>;
}
