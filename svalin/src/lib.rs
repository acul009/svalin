// The segregation I wanted to use here doesn't really make sense in practice.
// TODO: refactor module locations
pub mod agent;
pub mod client;
pub mod server;
pub mod shared;

pub mod verifier;

#[cfg(test)]
mod test;
// mod wip;
