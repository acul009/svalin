use crate::{SpkiHash, mls::key_package::KeyPackage};

pub trait KeyRetriever {
    type Error;

    fn get_key_packages_for_user(user: SpkiHash) -> Result<Vec<KeyPackage>, Self::Error>;
}
