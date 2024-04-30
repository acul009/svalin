use anyhow::{anyhow, Ok, Result};
use serde::{Deserialize, Serialize};
use svalin_pki::Certificate;

#[derive(Serialize, Deserialize)]
struct SavedUser {
    certificate: Certificate,
    username: String,
    encrypted_credentials: Vec<u8>,
    password_double_hash: Vec<u8>,
}

pub struct UserStore {
    scope: marmelade::Scope,
    root: Certificate,
}

impl UserStore {
    fn new(scope: marmelade::Scope, root: Certificate) -> Self {
        Self { scope, root }
    }

    fn get_user(&self, public_key: &[u8]) -> Result<Option<SavedUser>> {
        let mut user: Option<SavedUser> = None;

        self.scope.view(|b| {
            let b = b.get_bucket("userdata")?;
            user = b.get_object(public_key)?;

            Ok(())
        })?;

        Ok(user)
    }

    fn get_user_by_username(&self, username: &str) -> Result<Option<SavedUser>> {
        let mut user: Option<SavedUser> = None;

        self.scope.view(|b| {
            let usernames = b.get_bucket("usernames")?;

            let public_key_user: Option<Vec<u8>> = usernames.get_object(username)?;

            if let Some(public_key) = public_key_user {
                let b = b.get_bucket("userdata")?;
                user = b.get_object(&public_key)?;
            }

            Ok(())
        })?;

        Ok(user)
    }

    fn add_user(&self, user: SavedUser) -> Result<()> {
        self.scope.update(move |b| {
            let public_key = user.certificate.public_key();

            let usernames = b.get_bucket("usernames")?;

            if usernames.get_kv(&user.username).is_some() {
                return Err(anyhow!("Username already in use"));
            }

            let b = b.get_bucket("userdata")?;
            if b.get_kv(public_key).is_some() {
                return Err(anyhow!("User with uuid already exists"));
            }

            b.put_object(public_key.to_owned(), &user)?;

            usernames.put(user.username.clone(), public_key.to_owned())?;

            Ok(())
        })
    }
}
