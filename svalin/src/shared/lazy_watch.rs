use std::{
    ops::Deref,
    sync::{Arc, Mutex},
};

use tokio::sync::watch;

pub trait Handler: Send + 'static {
    type T;

    fn start(&mut self, send: &watch::Sender<Self::T>);
    fn stop(&mut self);
}

pub struct LazyWatch<T, H> {
    sender: watch::Sender<T>,
    handler: Arc<Mutex<HandlerWrapper<H>>>,
}

struct HandlerWrapper<H> {
    receiver_count: usize,
    handler: H,
}

impl<T, H> LazyWatch<T, H> {
    pub fn new(init: T, handler: H) -> Self {
        let (send, _recv) = watch::channel(init);
        Self {
            sender: send,
            handler: Arc::new(Mutex::new(HandlerWrapper {
                receiver_count: 0,
                handler,
            })),
        }
    }
}

impl<T, H> LazyWatch<T, H>
where
    H: Handler<T = T>,
{
    pub fn subscribe(&self) -> Receiver<T, H> {
        let mut lock = self.handler.lock().unwrap();

        let receiver = self.sender.subscribe();

        if lock.receiver_count == 0 {
            lock.handler.start(&self.sender)
        }

        lock.receiver_count += 1;

        Receiver {
            receiver,
            handler: self.handler.clone(),
        }
    }
}

pub struct Receiver<T, H>
where
    H: Handler<T = T>,
{
    receiver: watch::Receiver<T>,
    handler: Arc<Mutex<HandlerWrapper<H>>>,
}

impl<T, H> Deref for Receiver<T, H>
where
    H: Handler<T = T>,
{
    type Target = watch::Receiver<T>;

    fn deref(&self) -> &Self::Target {
        &self.receiver
    }
}

impl<T, H> Drop for Receiver<T, H>
where
    H: Handler<T = T>,
{
    fn drop(&mut self) {
        let mut lock = self.handler.lock().unwrap();

        lock.receiver_count -= 1;

        if lock.receiver_count == 0 {
            lock.handler.stop();
        }
    }
}
