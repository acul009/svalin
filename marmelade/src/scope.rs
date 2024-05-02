use crate::{
    transaction_type::{RoTransaction, RwTransaction},
    Bucket, DB,
};

#[derive(Clone)]
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

    pub fn view<CB: FnMut(Bucket<RoTransaction>) -> anyhow::Result<()>>(
        &self,
        mut f: CB,
    ) -> anyhow::Result<()> {
        let tx = self.db.jamm.tx(false)?;

        let bucket = tx.get_bucket(self.scope.as_bytes())?;

        let mut ro_bucket = Bucket {
            bucket: bucket,
            transaction_type: std::marker::PhantomData::<RoTransaction>,
        };

        for key in self.path.iter() {
            ro_bucket = ro_bucket.get_bucket(key.as_bytes())?;
        }

        f(ro_bucket)?;

        Ok(())
    }

    pub fn update<DB: FnMut(Bucket<RwTransaction>) -> anyhow::Result<()>>(
        &self,
        mut f: DB,
    ) -> anyhow::Result<()> {
        let tx = self.db.jamm.tx(true)?;

        let bucket = tx.get_bucket(self.scope.as_bytes())?;

        let mut rw_bucket = Bucket {
            bucket: bucket,
            transaction_type: std::marker::PhantomData::<RwTransaction>,
        };

        for key in self.path.iter() {
            rw_bucket = rw_bucket.get_bucket(key.as_bytes())?;
        }

        f(rw_bucket)?;

        tx.commit()?;

        Ok(())
    }

    pub fn subscope(&self, subscope: String) -> Self {
        let mut path = self.path.clone();
        path.push(subscope);
        Self {
            db: self.db.clone(),
            scope: self.scope.clone(),
            path,
        }
    }
}