use anyhow::{anyhow, Ok, Result};
use serde::{Deserialize, Serialize};
use svalin_pki::Certificate;

#[derive(Serialize, Deserialize)]
struct SavedUser {
    uuid: uuid::Uuid,
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

    fn get_user(&self, uuid: &uuid::Uuid) -> Result<Option<SavedUser>> {
        let mut user: Option<SavedUser> = None;

        self.scope.view(|b| {
            let b = b.get_bucket("userdata")?;
            user = b.get_object(uuid)?;

            Ok(())
        })?;

        Ok(user)
    }

    fn get_user_by_username(&self, username: &str) -> Result<Option<SavedUser>> {
        let mut user: Option<SavedUser> = None;

        self.scope.view(|b| {
            let usernames = b.get_bucket("usernames")?;

            let uuid: Option<uuid::Uuid> = usernames.get_object(username)?;

            if let Some(uuid) = uuid {
                let b = b.get_bucket("userdata")?;
                user = b.get_object(uuid)?;
            }

            Ok(())
        })?;

        Ok(user)
    }

    fn add_user(&self, user: SavedUser) -> Result<()> {
        self.scope.update(|b| {
            let usernames = b.get_bucket("usernames")?;

            if usernames.get_kv(&user.username).is_some() {
                return Err(anyhow!("Username already in use"));
            }

            let b = b.get_bucket("userdata")?;
            if b.get_kv(user.uuid).is_some() {
                return Err(anyhow!("User with uuid already exists"));
            }

            b.put_object(user.uuid.to_bytes_le(), &user)?;

            usernames.put_object(user.username.clone(), &user.uuid)?;

            Ok(())
        })
    }
}
