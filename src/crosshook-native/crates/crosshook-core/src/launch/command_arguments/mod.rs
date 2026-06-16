//! Curated game argv tokens and resolution for supported launch methods.

mod catalog;
mod resolver;

#[cfg(test)]
mod tests;

pub use catalog::{
    global_catalog, initialize_catalog, load_catalog, parse_catalog_toml, CommandArgumentCatalog,
    CommandArgumentEntry, DEFAULT_CATALOG_TOML,
};
pub use resolver::{
    is_known_command_argument_id, resolve_command_arguments, resolve_command_arguments_for_method,
    CommandArgumentResolveError, ResolvedCommandArguments,
};
