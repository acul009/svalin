use std::fmt::{Display, Formatter};

use jammdb::ToBytes;
use serde::{de::DeserializeOwned, Serialize};

use crate::{transaction_type::RwTransaction, Bucket};

#[derive(Debug)]
pub enum MarmeladeObjectError {
    Postcard(postcard::Error),
    Jammdb(jammdb::Error),
}

impl std::error::Error for MarmeladeObjectError {}

impl From<postcard::Error> for MarmeladeObjectError {
    fn from(e: postcard::Error) -> Self {
        MarmeladeObjectError::Postcard(e)
    }
}

impl From<jammdb::Error> for MarmeladeObjectError {
    fn from(e: jammdb::Error) -> Self {
        MarmeladeObjectError::Jammdb(e)
    }
}

impl Display for MarmeladeObjectError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MarmeladeObjectError::Postcard(p) => p.fmt(f),
            MarmeladeObjectError::Jammdb(j) => j.fmt(f),
        }
    }
}

impl<'b, 'tx, S> Bucket<'b, 'tx, S> {
    pub fn get_object<T: DeserializeOwned, U: AsRef<[u8]>>(
        &self,
        key: U,
    ) -> Result<Option<T>, postcard::Error> {
        if let Some(raw) = self.get_kv(key) {
            let decoded: T = postcard::from_bytes(raw.value())?;

            Ok(Some(decoded))
        } else {
            Ok(None)
        }
    }
}

impl<'b, 'tx> Bucket<'b, 'tx, RwTransaction> {
    pub fn put_object(
        &self,
        key: impl ToBytes<'tx>,
        value: &(impl Serialize + ?Sized),
    ) -> Result<(), MarmeladeObjectError> {
        let raw = postcard::to_extend(&value, Vec::new())?;

        self.bucket.put(key, raw)?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use serde::{Deserialize, Serialize};

    use crate::DB;

    #[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
    struct TestStruct {
        u64: u64,
        string: String,
    }

    #[test]
    fn test() {
        // delete old test db
        let _ = std::fs::remove_file("./marmelade_test.jammdb");
        let db = DB::open("./marmelade_test.jammdb").unwrap();

        let test = TestStruct {
            u64: 42,
            string: "test".to_string(),
        };

        let scope = db.scope("test".to_string()).unwrap();

        scope
            .update(|b| {
                b.put_object("test", &test)?;

                Ok(())
            })
            .unwrap();

        let mut test2: Option<TestStruct> = None;

        scope
            .view(|b| {
                test2 = b.get_object("test")?;

                Ok(())
            })
            .unwrap();

        assert_eq!(test, test2.unwrap());
    }
}
