use std::path::Path;

use crate::Scope;

#[derive(Clone)]
pub struct DB {
    pub(crate) jamm: jammdb::DB,
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
                tx.commit()?;
                Ok(())
            }

            //Forward error with any other cause
            Some(e) => Err(e),

            // Continue if the bucket was found
            None => Ok(()),
        }?;

        Ok(Scope::root_scope(self.clone(), name))
    }
}
