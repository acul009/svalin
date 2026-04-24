use std::fmt::{Debug, Display};

use crate::{
    SpkiHash,
    mls::{group_id::SvalinGroupId, key_package::UnverifiedKeyPackage},
};

pub trait KeyRetriever {
    type Error: Send + Sync + Display + Debug + 'static;

    fn get_required_group_members(
        &self,
        id: &SvalinGroupId,
    ) -> impl Future<Output = Result<Vec<SpkiHash>, Self::Error>>;
    fn get_key_packages(
        &self,
        entities: &[SpkiHash],
    ) -> impl Future<Output = Result<Vec<UnverifiedKeyPackage>, Self::Error>>;
}
