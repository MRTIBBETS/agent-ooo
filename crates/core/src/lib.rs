pub mod session_checkpoint_capnp {
    include!(concat!(env!("OUT_DIR"), "/session_checkpoint_capnp.rs"));
}

pub mod schema {
    pub use crate::session_checkpoint_capnp::*;
}

pub mod checkin;
pub mod detox;
pub mod reset;
pub mod discharge;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum OooError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Cap'n Proto error: {0}")]
    Capnp(#[from] capnp::Error),

    #[error("Transcript parse error: {0}")]
    TranscriptParse(String),

    #[error("Canary verification failed: {0}")]
    CanaryFailed(String),

    #[error("General error: {0}")]
    General(String),
}

pub type Result<T> = std::result::Result<T, OooError>;
