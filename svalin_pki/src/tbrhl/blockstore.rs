use std::future::Future;

use super::{block::Block, transaction::Transaction};

pub trait BlockStore {
    type T: Transaction;

    fn len(&self) -> u64;

    fn get(&self, index: u64) -> impl Future<Output = Option<Block<Self::T>>>;
    fn add(&mut self, block: Block<Self::T>);
}
