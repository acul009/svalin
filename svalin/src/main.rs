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
            tracing::debug!("User wants to run server");

            let address = address.parse().unwrap();
            tracing::debug!("Server address parsed successfully");
            let mutex = Arc::new(Mutex::<Option<Server>>::new(None));
            let mutex2 = mutex.clone();

            let cancel = CancellationToken::new();
            let cancel2 = cancel.clone();
            let cancel3 = cancel.clone();

            tokio::spawn(async move {
                tracing::debug!("Starting server");
                // This needs to be in a seperate task since the init server will block on
                // start_server
                let server = Server::build()
                    .addr(address)
                    .cancel(cancel2)
                    .start_server()
                    .await
                    .unwrap();

                *mutex2.lock().unwrap() = Some(server);
            });

            tokio::spawn(async move {
                // Wait for shutdown signal
                tokio::signal::ctrl_c().await.unwrap();

                cancel3.cancel();
            });

            cancel.cancelled().await;
            println!("Shutting down server...");

            let server = mutex.lock().unwrap().take();

            if let Some(server) = server {
                server.close(Duration::from_secs(5)).await.unwrap();
            } else {
                panic!("server Mutex was empty")
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
                agent.close(Duration::from_secs(1)).await.unwrap();
            }
            None => {
                let cancel = CancellationToken::new();
                let cancel2 = cancel.clone();
                let agent = Arc::new(Agent::open(cancel.clone()).await.unwrap());
                let agent2 = agent.clone();

                tokio::spawn(async move {
                    agent2.run().await.unwrap();
                });

                tokio::spawn(async move {
                    // Wait for shutdown signal
                    tokio::signal::ctrl_c().await.unwrap();

                    cancel2.cancel();
                });

                cancel.cancelled().await;
                println!("Shutting down agent");

                agent.close(Duration::from_secs(1)).await.unwrap();
            }
        },
    }
}
