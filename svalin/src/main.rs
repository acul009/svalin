use std::{net::ToSocketAddrs, os::unix::net::SocketAddr};

use svalin_rpc::*;

use clap::{Parser, Subcommand};

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
                    if let Ok(mut client) = Client::new(addr.next().unwrap()) {
                        let res = client.ping().await;
                        if let Err(err) = res {
                            println!("Err: {}", err)
                        }
                    }
                }
                Err(err) => {
                    println!("Err: {}", err);
                }
            }
        }
        Command::Server { address } => {
            if let Ok(addr) = address.parse() {
                if let Ok(mut server) = Server::new(addr) {
                    let res = server.run().await;
                    if let Err(err) = res {
                        println!("Err: {}", err)
                    }
                }
            }
        }
    }
}
