use std::{fmt::Debug, path::Path};

use crate::Scope;

#[derive(Clone)]
pub struct DB {
    pub(crate) jamm: jammdb::DB,
}

impl Debug for DB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DB").finish()
    }
}

impl DB {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<DB, jammdb::Error> {
        let jamm = jammdb::DB::open(path)?;
        Ok(DB { jamm })
    }

    pub fn list_scopes(&self) -> Result<Vec<String>, jammdb::Error> {
        let tx = self.jamm.tx(false)?;

        let mut scopes = Vec::new();
        for bucket in tx.buckets() {
            scopes.push(String::from_utf8_lossy(bucket.0.name()).to_string());
        }

        Ok(scopes)
    }

    pub fn scope(&self, name: String) -> Result<Scope, jammdb::Error> {
        {
            let tx = self.jamm.tx(true)?;
            tx.get_or_create_bucket(name.as_bytes())?;
            tx.commit()?;
        }

        Ok(Scope::root_scope(self.clone(), name))
    }

    pub fn delete_scope(&self, name: &str) -> Result<(), jammdb::Error> {
        let tx = self.jamm.tx(true)?;
        tx.delete_bucket(name.as_bytes())?;
        tx.commit()?;
        Ok(())
    }
}
