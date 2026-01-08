#![forbid(unsafe_code)]
pub use tracing_subscriber;

// The segregation I wanted to use here doesn't really make sense in practice.
// TODO: refactor module locations
pub mod agent;
pub mod async_com;
pub mod client;
pub mod device;
pub mod permissions;
pub mod server;
pub mod shared;
pub mod util;
pub mod verifier;

#[cfg(test)]
mod test;
// mod wip;
