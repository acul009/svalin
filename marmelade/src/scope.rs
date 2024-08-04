use anyhow::{Ok, Result};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    transaction_type::{RoTransaction, RwTransaction},
    Bucket, DB,
};

#[derive(Clone, Debug)]
pub struct Scope {
    db: DB,
    scope: String,
    path: Vec<String>,
}

impl Scope {
    pub(crate) fn root_scope(db: DB, scope: String) -> Self {
        Self {
            db,
            scope,
            path: vec![],
        }
    }

    fn new(db: DB, scope: String, path: Vec<String>) -> anyhow::Result<Self> {
        let tx = db.jamm.tx(true)?;

        let bucket = tx.get_bucket(scope.as_bytes())?;

        let mut rw_bucket = Bucket {
            bucket,
            transaction_type: std::marker::PhantomData::<RwTransaction>,
        };

        for key in path.iter() {
            rw_bucket = rw_bucket.get_or_create_bucket(key.as_bytes())?;
        }

        tx.commit()?;

        Ok(Self { db, scope, path })
    }

    pub fn view<CB: FnMut(Bucket<RoTransaction>) -> anyhow::Result<()>>(
        &self,
        mut f: CB,
    ) -> anyhow::Result<()> {
        let tx = self.db.jamm.tx(false)?;

        let bucket = tx.get_bucket(self.scope.as_bytes())?;

        let mut ro_bucket = Bucket {
            bucket,
            transaction_type: std::marker::PhantomData::<RoTransaction>,
        };

        for key in self.path.iter() {
            ro_bucket = ro_bucket.get_bucket(key.as_bytes())?;
        }

        f(ro_bucket)?;

        Ok(())
    }

    pub fn update<DB: FnOnce(Bucket<RwTransaction>) -> anyhow::Result<()>>(
        &self,
        f: DB,
    ) -> anyhow::Result<()> {
        let tx = self.db.jamm.tx(true)?;

        let bucket = tx.get_bucket(self.scope.as_bytes())?;

        let mut rw_bucket = Bucket {
            bucket,
            transaction_type: std::marker::PhantomData::<RwTransaction>,
        };

        for key in self.path.iter() {
            // This could also just be get_bucket, but making sure never hurts
            rw_bucket = rw_bucket.get_or_create_bucket(key.as_bytes())?;
        }

        f(rw_bucket)?;

        tx.commit()?;

        Ok(())
    }

    pub fn subscope(&self, subscope: String) -> Result<Self> {
        let mut path = self.path.clone();
        path.push(subscope);
        Self::new(self.db.clone(), self.scope.clone(), path)
    }

    pub fn get_object<T: DeserializeOwned, U: AsRef<[u8]>>(&self, key: U) -> Result<Option<T>> {
        let mut object: Option<T> = None;
        self.view(|b| {
            object = b.get_object(&key)?;

            Ok(())
        })?;

        Ok(object)
    }

    pub fn put_object<'a, U>(&'a self, key: String, value: &U) -> Result<()>
    where
        U: Serialize + ?Sized,
    {
        self.update(|b| {
            b.put_object(key, value)?;

            Ok(())
        })?;

        Ok(())
    }
}
