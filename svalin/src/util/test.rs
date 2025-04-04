use tokio_util::sync::CancellationToken;

use super::smart_subscriber::SmartSubscriber;

pub struct DataWrapper {
    // How the hell do I properly use this?
    data: SmartSubscriber<_, String>,
}

impl DataWrapper {
    fn new() -> Self {
        Self {
            data: SmartSubscriber::new(
                "".to_string(),
                CancellationToken::new(),
                |sender, cancel| async {
                    for i in 0..10 {
                        let _ = sender.send(i.to_string());
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                },
            ),
        }
    }
}
