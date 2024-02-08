use std::path::Path;

use jammdb::{Cursor, Data, KVPair, ToBytes};

#[derive(Clone)]
pub struct DB {
    jamm: jammdb::DB,
}

impl DB {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<DB, jammdb::Error> {
        let jamm = jammdb::DB::open(path)?;
        Ok(DB { jamm: jamm })
    }

    pub fn scope(&self, name: String) -> Result<Scope, jammdb::Error> {
        let check_bucket: Option<jammdb::Error>;
        {
            let tx = self.jamm.tx(false)?;
            check_bucket = match tx.get_bucket(name.as_bytes()) {
                Err(err) => Some(err),
                Ok(_) => None,
            };
        }

        match check_bucket {
            // Create Bucket if missing
            Some(jammdb::Error::BucketMissing) => {
                let tx = self.jamm.tx(true)?;
                tx.create_bucket(name.as_bytes())?;
                Ok(())
            }

            //Forward error with any other cause
            Some(e) => Err(e),

            // Continue if the bucket was found
            None => Ok(()),
        }?;

        Ok(Scope {
            db: self.clone(),
            scope: name,
            path: Vec::new(),
        })
    }
}

#[derive(Clone)]
pub struct Scope {
    db: DB,
    scope: String,
    path: Vec<String>,
}

impl Scope {
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
}

trait TransactionType {}
pub struct RoTransaction;
impl TransactionType for RoTransaction {}
pub struct RwTransaction;
impl TransactionType for RwTransaction {}

pub struct Bucket<'b, 'tx, TransactionType> {
    bucket: jammdb::Bucket<'b, 'tx>,
    transaction_type: std::marker::PhantomData<TransactionType>,
}

impl<'b, 'tx, S> Bucket<'b, 'tx, S> {
    pub fn get_bucket<'a, T: ToBytes<'tx>>(
        &'a self,
        name: T,
    ) -> Result<Bucket<'b, 'tx, S>, jammdb::Error> {
        let bucket = self.bucket.get_bucket(name)?;

        Ok(Bucket {
            bucket: bucket,
            transaction_type: self.transaction_type,
        })
    }

    pub fn get<'a, T: AsRef<[u8]>>(&'a self, key: T) -> Option<Data<'b, 'tx>> {
        self.bucket.get(key)
    }

    pub fn get_kv<'a, T: AsRef<[u8]>>(&'a self, key: T) -> Option<KVPair<'b, 'tx>> {
        self.bucket.get_kv(key)
    }

    pub fn cursor<'a>(&'a self) -> Cursor<'b, 'tx> {
        self.bucket.cursor()
    }

    pub fn next_int(&self) -> u64 {
        self.bucket.next_int()
    }
}

impl<'b, 'tx> Bucket<'b, 'tx, RwTransaction> {
    pub fn put<'a, T: ToBytes<'tx>, S: ToBytes<'tx>>(
        &'a self,
        key: T,
        value: S,
    ) -> Result<Option<KVPair<'b, 'tx>>, jammdb::Error> {
        self.bucket.put(key, value)
    }

    pub fn delete<T: AsRef<[u8]>>(&self, key: T) -> Result<KVPair, jammdb::Error> {
        self.bucket.delete(key)
    }

    pub fn create_bucket<'a, T: ToBytes<'tx>>(
        &'a self,
        name: T,
    ) -> Result<Bucket<'b, 'tx, RwTransaction>, jammdb::Error> {
        let bucket = self.bucket.create_bucket(name)?;

        Ok(Bucket {
            bucket: bucket,
            transaction_type: self.transaction_type,
        })
    }

    pub fn get_or_create_bucket<'a, T: ToBytes<'tx>>(
        &'a self,
        name: T,
    ) -> Result<Bucket<'b, 'tx, RwTransaction>, jammdb::Error> {
        let bucket = self.bucket.get_or_create_bucket(name)?;

        Ok(Bucket {
            bucket: bucket,
            transaction_type: self.transaction_type,
        })
    }

    pub fn delete_bucket<T: ToBytes<'tx>>(&self, key: T) -> Result<(), jammdb::Error> {
        self.bucket.delete_bucket(key)
    }
}
