use std::net::ToSocketAddrs;

use clap::{Parser, Subcommand};
use svalin::Server;
use svalin_rpc::Client;

#[derive(Debug, Parser)]
#[clap(name = "my-app", version)]
pub struct App {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Client { address: String },
    Server { address: String },
}

fn main() {
    println!("Hello, world!");
    run();
}

#[tokio::main]
async fn run() {
    let app = App::parse();

    match app.command {
        Command::Client { address } => {
            println!("trying to run client");
            match address.to_socket_addrs() {
                Ok(mut addr) => {
                    // if let Ok(mut client) = Client::new(addr.next().unwrap()) {
                    //     let res = client.ping().await;
                    //     if let Err(err) = res {
                    //         println!("Err: {}", err)
                    //     }
                    // }
                }
                Err(err) => {
                    println!("Err: {}", err);
                }
            }
        }
        Command::Server { address } => {
            if let Ok(addr) = address.parse() {
                let db = marmelade::DB::open("./server.jammdb").expect("failed to open client db");
                Server::run(addr, todo!());
            }
        }
    }
}
