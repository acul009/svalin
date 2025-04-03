use std::fmt;
use std::{borrow::Cow, hash::Hash};

use core::pin::Pin;
use core::task::{Context, Poll, ready};
use futures::Stream;
use futures_util::StreamExt;
use iced::advanced::{graphics::futures::boxed_stream, subscription::Recipe};
use tokio::sync::watch;
use tokio_util::sync::ReusableBoxFuture;

#[derive(Debug)]
pub struct WatchRecipe<I, T, Message>
where
    I: Clone + 'static,
{
    id: Cow<'static, I>,
    watcher: watch::Receiver<T>,
    message: Message,
}

impl<I, T, Message> Clone for WatchRecipe<I, T, Message>
where
    I: Clone + 'static,
    Message: Clone + 'static,
{
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            watcher: self.watcher.clone(),
            message: self.message.clone(),
        }
    }
}

impl<I, T, M> WatchRecipe<I, T, M>
where
    I: Clone + 'static,
{
    pub fn new(id: I, watcher: watch::Receiver<T>, message: M) -> Self {
        Self {
            id: Cow::Owned(id),
            watcher,
            message,
        }
    }

    pub fn borrow(&self) -> watch::Ref<'_, T> {
        self.watcher.borrow()
    }
}

impl<I, T, Message> Recipe for WatchRecipe<I, T, Message>
where
    I: Clone + Hash + Send + Sync + 'static,
    T: Send + Sync + 'static,
    Message: Clone + Send + Sync + 'static,
{
    type Output = Message;

    fn hash(&self, state: &mut iced::advanced::subscription::Hasher) {
        self.id.hash(state);
    }

    fn stream(
        self: Box<Self>,
        _input: iced::advanced::subscription::EventStream,
    ) -> iced::advanced::graphics::futures::BoxStream<Self::Output> {
        let message = self.message;
        let watch_stream = WatchNotifyStream::new(self.watcher);
        let mapped = watch_stream.map(move |_| message.clone());
        boxed_stream(mapped)
    }
}

pub struct WatchNotifyStream<T> {
    inner: ReusableBoxFuture<'static, (Result<(), watch::error::RecvError>, watch::Receiver<T>)>,
}

impl<T: Send + Sync + 'static> WatchNotifyStream<T> {
    /// Create a new `WatchNotifyStream`
    pub fn new(rx: watch::Receiver<T>) -> Self {
        Self {
            inner: ReusableBoxFuture::new(make_future(rx)),
        }
    }
}

async fn make_future<T>(
    mut rx: watch::Receiver<T>,
) -> (Result<(), watch::error::RecvError>, watch::Receiver<T>) {
    let result = rx.changed().await;
    (result, rx)
}

impl<T: 'static + Send + Sync> Stream for WatchNotifyStream<T> {
    type Item = ();

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let (result, rx) = ready!(self.inner.poll(cx));
        match result {
            Ok(_) => {
                self.inner.set(make_future(rx));
                Poll::Ready(Some(()))
            }
            Err(_) => {
                self.inner.set(make_future(rx));
                Poll::Ready(None)
            }
        }
    }
}

impl<T> Unpin for WatchNotifyStream<T> {}

impl<T> fmt::Debug for WatchNotifyStream<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WatchNotifyStream").finish()
    }
}

impl<T: 'static + Clone + Send + Sync> From<watch::Receiver<T>> for WatchNotifyStream<T> {
    fn from(recv: watch::Receiver<T>) -> Self {
        Self::new(recv)
    }
}
