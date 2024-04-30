use jammdb::{Cursor, Data, KVPair, ToBytes};

use crate::transaction_type::RwTransaction;

pub struct Bucket<'b, 'tx, TransactionType> {
    pub(crate) bucket: jammdb::Bucket<'b, 'tx>,
    pub(crate) transaction_type: std::marker::PhantomData<TransactionType>,
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
