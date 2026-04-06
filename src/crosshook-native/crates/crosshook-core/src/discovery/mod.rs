//! Trainer discovery domain: manifest parsing, search query/response, and version matching.

pub mod client;
pub mod matching;
pub mod models;
pub mod search;

pub use client::search_external_trainers;
pub use models::*;
pub use search::search_trainer_sources;
