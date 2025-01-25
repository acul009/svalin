use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use clap::{Parser, Subcommand};
use svalin::{agent::Agent, server::Server};

use tokio_util::sync::CancellationToken;
use tracing_subscriber;

#[derive(Debug, Parser)]
#[clap(name = "svalin", version)]
pub struct App {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run in server mode
    Server { address: String },
    /// Commands for running the agent
    Agent {
        #[clap(subcommand)]
        action: Option<AgentAction>,
    },
}

#[derive(Debug, Subcommand)]
enum AgentAction {
    /// Initialize the agent by connecting to a server
    Init { address: String },
}

fn main() {
    tracing_subscriber::fmt::init();
    run();
}

#[tokio::main]
async fn run() {
    let app = App::parse();

    match app.command {
        Command::Server { address } => {
            if let Ok(addr) = address.parse() {
                let db = marmelade::DB::open("./server.jammdb").expect("failed to open client db");

                let mutex = Arc::new(Mutex::<Option<Server>>::new(None));
                let mutex2 = mutex.clone();

                let cancel = CancellationToken::new();
                let cancel2 = cancel.clone();

                tokio::spawn(async move {
                    let server = Server::build()
                        .addr(addr)
                        .scope(db.scope("default".into()).unwrap())
                        .cancel(cancel2)
                        .start_server()
                        .await
                        .unwrap();

                    *mutex2.lock().unwrap() = Some(server);
                });

                // Wait for shutdown signal
                tokio::signal::ctrl_c().await.unwrap();

                println!("Shutting down server...");
                cancel.cancel();

                let server = mutex.lock().unwrap().take();

                if let Some(server) = server {
                    server.close(Duration::from_secs(5)).await.unwrap();
                } else {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
        Command::Agent { action } => match action {
            Some(AgentAction::Init { address }) => {
                let mut welcome_message = "-".repeat(40);
                welcome_message.push_str("Svalin Agent");
                welcome_message.push_str("-".repeat(40).as_str());
                println!("{welcome_message}");

                println!("connecting to {address}...");

                let waiting_for_init = Agent::init(address).await.unwrap();

                println!("Successfully requested to join server.");
                println!("Join-Code: {}", waiting_for_init.join_code());
                let waiting_for_confirm = waiting_for_init.wait_for_init().await.unwrap();
                println!("Confirm-Code: {}", waiting_for_confirm.confirm_code());
                let agent = waiting_for_confirm.wait_for_confirm().await.unwrap();
                println!("initialisation complete!");
                agent.close();
            }
            None => {
                let agent = Agent::open().await.unwrap();
                agent.run().await.unwrap();
            }
        },
    }
}
