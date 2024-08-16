pub use anyhow::anyhow;
pub use anyhow::Result;
pub use svalin::client::{device::Device, Client, FirstConnect, Init, Login};
pub use tokio::sync::broadcast::Receiver;

use crate::frb_generated::StreamSink;

macro_rules! create_streamsink_converter {
    ($t:ty) => {
        paste::paste! {
            pub async fn [<$t _receiver_into_streamsink>](
                receiver: Receiver<$t>,
                sink: StreamSink<$t>,
            ) {
                while let Ok(msg) = receiver.recv().await {
                    if let Err(_err) = sink.add(msg) {
                        return;
                    }
                }
            }
        }
    };
}

create_streamsink_converter!(Device);

// pub async fn DeviceListItem_receiver_into_streamsink(
//     receiver: Receiver<DeviceListItem>,
//     sink: StreamSink<DeviceListItem>,
// ) {
//     while let Ok(msg) = receiver.recv().await {
//         if let Err(err) = sink.add(msg) {
//             return;
//         }
//     }
// }

// pub trait ToStreamSink {
//     type T: Clone;

//     async fn streamsink(&mut self, sink: StreamSink<Self::T>) -> Result<()>;
// }

// impl<T> ToStreamSink for tokio::sync::broadcast::Receiver<T>
// where
//     T: Clone + SseEncode,
// {
//     type T = T;

//     async fn streamsink(&mut self, sink: StreamSink<Self::T>) -> Result<()> {
//         while let Ok(msg) = self.recv().await {
//             if let Err(err) = sink.add(msg) {
//                 return Err(anyhow!(err));
//             }
//         }

//         Ok(())
//     }
// }
