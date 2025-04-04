use std::sync::Mutex;

use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

pub trait SubscriberStarter: Sized + Send + 'static {
    type Item: Send + Sync + 'static;
    fn start(
        &self,
        send: watch::Sender<Self::Item>,
        cancel: CancellationToken,
    ) -> impl Future<Output = ()> + Send + 'static;
    fn default(&self) -> Self::Item;
}

pub struct SmartSubscriber<S: SubscriberStarter> {
    sender: Mutex<watch::Sender<S::Item>>,
    cancel: CancellationToken,
    starter: S,
}

impl<S> SmartSubscriber<S>
where
    S: SubscriberStarter,
{
    pub fn new(starter: S, cancel: CancellationToken) -> Self {
        Self {
            sender: Mutex::new(watch::channel(starter.default()).0),
            cancel,
            starter,
        }
    }

    pub fn subscribe(&self) -> watch::Receiver<S::Item> {
        let sender = {
            let sender = self.sender.lock().unwrap();

            // 2 Senders mean, there is already a background task running
            if sender.sender_count() >= 2 {
                return sender.subscribe();
            }

            sender.clone()
        };

        let recv = sender.subscribe();
        let cancel = self.cancel.clone();

        let future = self.starter.start(sender, cancel);
        tokio::spawn(future);

        recv
    }

    pub fn restart_if_offline(&self) {
        let receiver_count = { self.sender.lock().unwrap().receiver_count() };

        if receiver_count > 0 {
            self.subscribe();
        }
    }
}
