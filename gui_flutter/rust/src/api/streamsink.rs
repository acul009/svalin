use anyhow::anyhow;
pub use anyhow::Result;

use crate::frb_generated::{SseEncode, StreamSink};

pub trait ToStreamSink {
    type T: Clone;

    async fn streamsink(&mut self, sink: StreamSink<Self::T>) -> Result<()>;
}

impl<T> ToStreamSink for tokio::sync::broadcast::Receiver<T>
where
    T: Clone + SseEncode,
{
    type T = T;

    async fn streamsink(&mut self, sink: StreamSink<Self::T>) -> Result<()> {
        while let Ok(msg) = self.recv().await {
            if let Err(err) = sink.add(msg) {
                return Err(anyhow!(err));
            }
        }

        Ok(())
    }
}
