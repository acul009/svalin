use anyhow::Result;

use crate::Certificate;

use super::transaction::Transaction;

/// While this could be implemented using just state on the first glance, the
/// indirection using this handler allows to attach additional information that
/// can be used during verification.
///
/// e.g. You can add constraints who can sign the first transaction based on
/// outside info
pub trait TransactionHandler {
    type T: Transaction;
    type State;

    fn verify(
        &self,
        state: &Self::State,
        transaction: &Self::T,
        signer: &Certificate,
    ) -> Result<()>;

    fn apply(&self, state: &mut Self::State, transaction: &Self::T) -> Result<()>;

    fn init(&self, transaction: &Self::T, signer: &Certificate) -> Result<Self::State>;
}
