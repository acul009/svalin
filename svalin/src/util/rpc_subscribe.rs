use serde::{Serialize, de::DeserializeOwned};
use svalin_rpc::rpc::command::handler::CommandHandler;
use tokio::sync::{broadcast, watch};

pub trait Patchable: Serialize + DeserializeOwned {
    type Patch: Serialize + DeserializeOwned;

    fn apply(&mut self, patch: Self::Patch);
}

pub trait PatchableManager {
    type Item: Patchable;

    fn subscribe(
        &self,
    ) -> anyhow::Result<(
        Self::Item,
        broadcast::Receiver<<Self::Item as Patchable>::Patch>,
    )>;
}
