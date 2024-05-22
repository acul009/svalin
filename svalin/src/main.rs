use clap::{Parser, Subcommand};
use svalin::{agent::Agent, server::Server};

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
        action: AgentAction,
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
                let mut server = Server::prepare(addr, db.scope("default".into()).unwrap())
                    .await
                    .unwrap();

                server.run().await.unwrap();
            }
        }
        Command::Agent { action } => match action {
            AgentAction::Init { address } => {
                Agent::init(address).await.unwrap();
            }
        },
    }
}
