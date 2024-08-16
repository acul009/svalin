use std::{
    future::Future,
    sync::{atomic::AtomicUsize, Arc},
};

use tokio::sync::{watch, Mutex};

pub trait Handler: Send + 'static {
    type T;

    fn start(&mut self, send: watch::Sender<Self::T>) -> impl Future<Output = ()> + Send;
    fn stop(&mut self) -> impl Future<Output = ()> + Send;
}

struct Sender<T, H> {
    send: watch::Sender<T>,
    recv: watch::Receiver<T>,
    handler: Arc<Mutex<HandlerWrapper<H>>>,
}

struct HandlerWrapper<H> {
    count: usize,
    handler: H,
}

struct Watcher<T, H>
where
    H: Handler<T = T>,
{
    recv: watch::Receiver<T>,
    handler: Arc<Mutex<HandlerWrapper<H>>>,
}

impl<T, H> Sender<T, H>
where
    H: Handler<T = T>,
{
    pub fn new(handler: H, init: T) -> Self {
        let (send, recv) = watch::channel(init);
        Sender {
            send: send,
            recv: recv,
            handler: Arc::new(Mutex::new(HandlerWrapper {
                count: 0,
                handler: handler,
            })),
        }
    }

    pub async fn watch(&self) -> Watcher<T, H> {
        let mut lock = self.handler.lock().await;

        if lock.count == 0 {
            lock.handler.start(self.send.clone()).await;
        }

        lock.count += 1;

        Watcher {
            recv: self.recv.clone(),
            handler: self.handler.clone(),
        }
    }
}

impl<T, H> Drop for Watcher<T, H>
where
    H: Handler<T = T>,
{
    fn drop(&mut self) {
        let handler = self.handler.clone();
        tokio::spawn(async move {
            let mut lock = handler.lock().await;

            lock.count -= 1;

            if lock.count == 0 {
                lock.handler.stop().await;
            }
        });
    }
}
