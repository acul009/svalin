use std::fmt::{write, Display};

#[derive(Debug)]
pub enum Error {
    Unrecoverable(String),
    KeyMismatch,
}

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unrecoverable(msg) => write!(f, "{}", msg),
            Self::KeyMismatch => write!(f, "given key does not match expected key"),
        }
    }
}
