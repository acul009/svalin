use std::convert::Infallible;

use serde::{Deserialize, Serialize};

use crate::{
    Credential,
    secure_chain::{self, Chain},
};

#[derive(Debug)]
struct State {
    number: u64,
    allow_wrong_transaction: bool,
}

#[derive(Serialize, Deserialize)]
struct Exported {
    number: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transaction {
    add: u64,
}

impl secure_chain::Transaction for Transaction {
    fn digest(&self, digest: &mut impl sha2::Digest) {
        digest.update(self.add.to_le_bytes());
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("transaction too big")]
    TooBig,
    #[error("transaction too small")]
    TooSmall,
}

impl secure_chain::ChainState for State {
    type Transaction = Transaction;

    type Error = Error;
    type Exported = Exported;
    type ImportError = Infallible;

    fn check(
        &self,
        _signer: &crate::SpkiHash,
        _time: u64,
        transaction: &Self::Transaction,
    ) -> Result<(), Self::Error> {
        if self.allow_wrong_transaction {
            return Ok(());
        }
        if transaction.add > 10 {
            Err(Error::TooBig)
        } else if transaction.add < 1 {
            Err(Error::TooSmall)
        } else {
            Ok(())
        }
    }

    fn apply(&mut self, _signer: &crate::SpkiHash, _time: u64, transaction: &Self::Transaction) {
        self.number += transaction.add;
    }

    fn revert(&mut self, transaction: &Self::Transaction) {
        self.number -= transaction.add;
    }

    fn digest(&self, digest: &mut impl sha2::Digest) {
        digest.update(self.number.to_le_bytes());
    }

    fn export(&self) -> Self::Exported {
        Self::Exported {
            number: self.number,
        }
    }

    fn import(exported: Self::Exported) -> Result<Self, Self::ImportError> {
        Ok(Self {
            number: exported.number,
            allow_wrong_transaction: false,
        })
    }
}

#[test]
fn test_simple_chain() {
    let credential = Credential::generate_root().unwrap();
    let state = State {
        number: 0,
        allow_wrong_transaction: true,
    };
    let mut chain = secure_chain::Chain::initialize(state);
    for _ in 0..100 {
        let transaction = Transaction {
            add: rand::random_range(1..=10),
        };
        let packaged = chain.package(transaction, &credential).unwrap();
        chain.apply(packaged);
    }

    let exported = chain.export();

    let mut saved = Vec::new();
    for _ in 100..200 {
        let transaction = Transaction {
            add: rand::random_range(1..=10),
        };
        let packaged = chain.package(transaction, &credential).unwrap();
        saved.push(packaged.as_unchecked().clone());
        chain.apply(packaged);
    }

    let mut chain2: secure_chain::Chain<State> = Chain::import(exported).unwrap();

    for packaged in saved.into_iter() {
        let checked = chain2.check(packaged, credential.certificate()).unwrap();
        chain2.apply(checked);
    }
    assert_eq!(chain.state().number, chain2.state().number);
    assert_eq!(chain.digest(), chain2.digest());

    let broken = chain
        .package(Transaction { add: 100 }, &credential)
        .unwrap()
        .as_unchecked()
        .clone();
    chain2.check(broken, credential.certificate()).unwrap_err();
}
