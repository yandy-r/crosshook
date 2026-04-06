//! Trainer discovery domain: manifest parsing, search query/response, and version matching.

pub mod models;
pub mod search;

pub use models::*;
pub use search::search_trainer_sources;
