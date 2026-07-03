//! Trainer discovery domain: manifest parsing, aggregated catalog, and version matching.

pub mod catalog;
pub mod client;
pub mod matching;
pub mod models;
pub mod ranking;

pub use catalog::{
    build_catalog_page, CatalogEntry, CatalogFacets, CatalogPage, CatalogQuery, CatalogRowInput,
    CatalogSource, CatalogSourceInput, FacetValue,
};
pub use client::search_external_trainers;
pub use models::*;
