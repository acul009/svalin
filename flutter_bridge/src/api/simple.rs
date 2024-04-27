use std::time::{Duration, SystemTime};

use anyhow::Result;
use svalin::client::{Client, FirstConnect};

use crate::frb_generated::StreamSink;

#[flutter_rust_bridge::frb(sync)] // Synchronous mode for simplicity of the demo
pub fn greet(name: String) -> String {
    format!("Hello, {name}!")
}

pub async fn stream_time(sink: StreamSink<String>) {
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let time = SystemTime::now();
        let time_format = format!("{:?}", time);

        if let Err(e) = sink.add(time_format) {
            println!("Error: {}", e);
            break;
        }
    }
}

#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    // Default utilities - feel free to customize
    flutter_rust_bridge::setup_default_user_utils();
}

pub fn list_profiles() -> Vec<String> {
    Client::get_profiles().unwrap()
}

pub async fn first_connect(address: String) -> Result<FirstConnect> {
    Client::first_connect(address).await
}
