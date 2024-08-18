pub use anyhow::anyhow;
pub use anyhow::Result;
pub use svalin::client::device::RemoteLiveData;
pub use svalin::client::{device::Device, Client, FirstConnect, Init, Login};
use svalin_sysctl::realtime::RealtimeStatus;

use crate::frb_generated::StreamSink;

macro_rules! create_broadcast_converter {
    ($t:ty) => {
        paste::paste! {
            pub async fn [<$t _broadcast_into_streamsink>](
                mut receiver: tokio::sync::broadcast::Receiver<$t>,
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

macro_rules! create_watcher_converter {
    ($t:ty) => {
        paste::paste! {
            pub async fn [<$t _watcher_into_streamsink>](
                mut receiver: tokio::sync::watch::Receiver<$t>,
                sink: frb_generated::StreamSink<$t>,
            ) {
                if let Err(_) = sink.add(receiver.borrow().clone()) {
                    return;
                }
                while let Ok(_) = receiver.changed().await {
                    if let Err(_) = sink.add(receiver.borrow().clone()) {
                        return;
                    }
                }
            }
        }
    };
}

// create_watcher_converter!(RealtimeStatus);
