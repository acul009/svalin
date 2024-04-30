pub(crate) trait TransactionType {}
pub struct RoTransaction;
impl TransactionType for RoTransaction {}
pub struct RwTransaction;
impl TransactionType for RwTransaction {}
