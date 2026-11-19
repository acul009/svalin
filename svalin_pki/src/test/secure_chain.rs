use std::convert::Infallible;

use serde::{Deserialize, Serialize};
use structured_digest::prelude::*;

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

impl secure_chain::State for State {
    type Transaction = Transaction;

    type Error = Error;
    type Exported = Exported;
    type ImportError = Infallible;

    fn check(
        &self,
        _signer: &crate::Certificate,
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

    fn apply(&mut self, _signer: &crate::Certificate, _time: u64, transaction: &Self::Transaction) {
        self.number += transaction.add;
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

impl Digestable for State {
    fn digest(&self, hasher: &mut impl structured_digest::Hasher) -> structured_digest::Finished {
        hasher.digest_struct().field("number", self.number).finish()
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
        let previous_number = chain.state().number;
        let packaged = chain.package(transaction, &credential).unwrap();
        assert_eq!(chain.state().number, previous_number);
        chain.apply(packaged);
    }

    let exported = chain.export();

    let mut saved = Vec::new();
    for _ in 100..200 {
        let transaction = Transaction {
            add: rand::random_range(1..=10),
        };
        let previous_number = chain.state().number;
        let packaged = chain.package(transaction, &credential).unwrap();
        assert_eq!(chain.state().number, previous_number);
        saved.push(packaged.pack().clone());
        chain.apply(packaged);
    }

    let mut chain2: secure_chain::Chain<State> = Chain::import(exported).unwrap();

    for packaged in saved.into_iter() {
        let previous_number = chain2.state().number;
        let checked = chain2
            .check(packaged, credential.certificate().clone())
            .unwrap();
        assert_eq!(chain2.state().number, previous_number);
        chain2.apply(checked);
    }
    assert_eq!(chain.state().number, chain2.state().number);
    assert_eq!(chain.sequence(), chain2.sequence());

    let broken = chain
        .package(Transaction { add: 100 }, &credential)
        .unwrap()
        .pack()
        .clone();
    chain2
        .check(broken, credential.certificate().clone())
        .unwrap_err();
}
