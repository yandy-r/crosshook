pub mod discovery;
pub mod flatpak;
pub mod matching;
pub mod resolution;
pub mod types;
mod util;

#[cfg(test)]
mod tests;

// Re-export everything previously public from the monolithic file so all
// existing `use crate::steam::proton::X` paths continue to resolve.

pub use discovery::{collect_compat_tool_mappings, discover_compat_tools};
pub use flatpak::prefer_user_local_compat_tool_path;
pub use resolution::resolve_proton_path;
pub use types::{CompatToolMappings, ProtonResolution};

// `pub(crate)` items — re-export with the same visibility
pub(crate) use matching::normalize_alias;
