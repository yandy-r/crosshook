use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{CommunityProfileManifest, COMMUNITY_PROFILE_SCHEMA_VERSION};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CommunityProfileIndex {
    pub entries: Vec<CommunityProfileIndexEntry>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityProfileIndexEntry {
    pub tap_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tap_branch: Option<String>,
    pub tap_path: PathBuf,
    pub manifest_path: PathBuf,
    pub relative_path: PathBuf,
    pub manifest: CommunityProfileManifest,
}

#[derive(Debug)]
pub enum CommunityProfileIndexError {
    Io {
        action: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
}

impl fmt::Display for CommunityProfileIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                action,
                path,
                source,
            } => write!(f, "failed to {action} '{}': {source}", path.display()),
            Self::Json { path, source } => write!(
                f,
                "failed to parse community profile '{}': {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for CommunityProfileIndexError {}

pub fn index_taps(
    workspaces: &[crate::community::taps::CommunityTapWorkspace],
) -> Result<CommunityProfileIndex, CommunityProfileIndexError> {
    let mut index = CommunityProfileIndex::default();

    for workspace in workspaces {
        let workspace_index = index_tap(workspace)?;
        index.entries.extend(workspace_index.entries);
        index.diagnostics.extend(workspace_index.diagnostics);
    }

    sort_entries(&mut index.entries);
    Ok(index)
}

pub fn index_tap(
    workspace: &crate::community::taps::CommunityTapWorkspace,
) -> Result<CommunityProfileIndex, CommunityProfileIndexError> {
    let mut index = CommunityProfileIndex::default();

    if !workspace.local_path.exists() {
        index.diagnostics.push(format!(
            "tap '{}' is not synced yet: {}",
            workspace.subscription.url,
            workspace.local_path.display()
        ));
        return Ok(index);
    }

    collect_manifests(
        &workspace.local_path,
        &workspace.local_path,
        workspace,
        &mut index,
    )?;

    sort_entries(&mut index.entries);
    Ok(index)
}

fn collect_manifests(
    root: &Path,
    current_dir: &Path,
    workspace: &crate::community::taps::CommunityTapWorkspace,
    index: &mut CommunityProfileIndex,
) -> Result<(), CommunityProfileIndexError> {
    for entry in fs::read_dir(current_dir).map_err(|source| CommunityProfileIndexError::Io {
        action: "read tap directory",
        path: current_dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| CommunityProfileIndexError::Io {
            action: "read tap directory entry",
            path: current_dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();

        if path.is_dir() {
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with('.'))
            {
                continue;
            }
            collect_manifests(root, &path, workspace, index)?;
            continue;
        }

        if path.file_name().and_then(|value| value.to_str()) != Some("community-profile.json") {
            continue;
        }

        let content =
            fs::read_to_string(&path).map_err(|source| CommunityProfileIndexError::Io {
                action: "read community profile JSON",
                path: path.clone(),
                source,
            })?;
        let manifest: CommunityProfileManifest =
            serde_json::from_str(&content).map_err(|source| CommunityProfileIndexError::Json {
                path: path.clone(),
                source,
            })?;

        if manifest.schema_version != COMMUNITY_PROFILE_SCHEMA_VERSION {
            index.diagnostics.push(format!(
                "skipping unsupported schema version {} at {}",
                manifest.schema_version,
                path.display()
            ));
            continue;
        }

        let relative_path = path
            .strip_prefix(root)
            .unwrap_or(path.as_path())
            .to_path_buf();

        index.entries.push(CommunityProfileIndexEntry {
            tap_url: workspace.subscription.url.clone(),
            tap_branch: workspace.subscription.branch.clone(),
            tap_path: workspace.local_path.clone(),
            manifest_path: path,
            relative_path,
            manifest,
        });
    }

    Ok(())
}

fn sort_entries(entries: &mut [CommunityProfileIndexEntry]) {
    entries.sort_by(|left, right| {
        left.manifest
            .metadata
            .game_name
            .cmp(&right.manifest.metadata.game_name)
            .then(left.manifest_path.cmp(&right.manifest_path))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::community::taps::{CommunityTapSubscription, CommunityTapWorkspace};
    use crate::community::{CommunityProfileMetadata, CompatibilityRating};
    use crate::profile::GameProfile;
    use tempfile::tempdir;

    fn sample_workspace(path: PathBuf) -> CommunityTapWorkspace {
        CommunityTapWorkspace {
            subscription: CommunityTapSubscription {
                url: "https://example.invalid/taps/community.git".to_string(),
                branch: Some("main".to_string()),
                pinned_commit: None,
            },
            local_path: path,
        }
    }

    #[test]
    fn indexes_nested_community_manifests() {
        let temp_dir = tempdir().unwrap();
        let tap_root = temp_dir.path().join("tap");
        let nested = tap_root.join("profiles").join("elden-ring");
        fs::create_dir_all(&nested).unwrap();

        let manifest = CommunityProfileManifest::new(
            CommunityProfileMetadata {
                game_name: "Elden Ring".to_string(),
                game_version: "1.0".to_string(),
                trainer_name: "Trainer".to_string(),
                trainer_version: "1".to_string(),
                proton_version: "9".to_string(),
                platform_tags: vec!["linux".to_string()],
                compatibility_rating: CompatibilityRating::Working,
                author: "CrossHook".to_string(),
                description: "Test profile".to_string(),
            },
            GameProfile::default(),
        );
        let manifest_path = nested.join("community-profile.json");
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();

        let index = index_tap(&sample_workspace(tap_root)).unwrap();
        assert_eq!(index.entries.len(), 1);
        assert_eq!(index.entries[0].manifest_path, manifest_path);
        assert_eq!(
            index.entries[0].relative_path,
            PathBuf::from("profiles/elden-ring/community-profile.json")
        );
    }

    #[test]
    fn supports_indexing_multiple_taps() {
        let temp_dir = tempdir().unwrap();
        let first = temp_dir.path().join("tap-a");
        let second = temp_dir.path().join("tap-b");
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();

        let manifest = CommunityProfileManifest::new(
            CommunityProfileMetadata {
                game_name: "B".to_string(),
                game_version: String::new(),
                trainer_name: String::new(),
                trainer_version: String::new(),
                proton_version: String::new(),
                platform_tags: vec![],
                compatibility_rating: CompatibilityRating::Unknown,
                author: String::new(),
                description: String::new(),
            },
            GameProfile::default(),
        );
        fs::write(
            first.join("community-profile.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        fs::write(
            second.join("community-profile.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();

        let index = index_taps(&[sample_workspace(first), sample_workspace(second)]).unwrap();
        assert_eq!(index.entries.len(), 2);
    }
}
