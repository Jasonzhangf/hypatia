pub mod cli;
pub mod embed;
pub mod engine;
pub mod error;
pub mod hybrid;
pub mod lab;
pub mod miner;
pub mod model;
pub mod service;
pub mod storage;
pub mod vector;

pub use embed::Embedder;
pub use error::{HypatiaError, Result};
pub use hybrid::HybridResult;
pub use lab::Lab;
pub use miner::{Chunk, Chunker, MinerConfig, Scanner, Watcher};
pub use vector::VectorStore;
