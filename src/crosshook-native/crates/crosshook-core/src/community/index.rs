use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{CommunityProfileManifest, COMMUNITY_PROFILE_SCHEMA_VERSION};
use crate::discovery::models::TrainerSourcesManifest;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CommunityProfileIndex {
    pub entries: Vec<CommunityProfileIndexEntry>,
    pub diagnostics: Vec<String>,
    /// Trainer source manifests discovered alongside community profiles.
    /// Each tuple is `(relative_path, manifest)` relative to the tap workspace root.
    pub trainer_sources: Vec<(String, TrainerSourcesManifest)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
        index
            .trainer_sources
            .extend(workspace_index.trainer_sources);
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
        let file_type = entry
            .file_type()
            .map_err(|source| CommunityProfileIndexError::Io {
                action: "read tap directory entry type",
                path: current_dir.to_path_buf(),
                source,
            })?;

        // file_type does not follow links: never read or recurse through a
        // symlink, so tap content cannot redirect the indexer outside the tap.
        if file_type.is_symlink() {
            tracing::warn!(
                path = %entry.path().display(),
                "skipping symlink entry in community tap"
            );
            continue;
        }

        let path = entry.path();
        if file_type.is_dir() {
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

        // Only regular files may be manifest names: a FIFO, socket or device
        // named `community-profile.json` must never be opened (a FIFO read
        // would block the indexer).
        if !file_type.is_file() {
            tracing::warn!(
                path = %entry.path().display(),
                "skipping non-regular-file entry in community tap"
            );
            continue;
        }

        let file_name = path.file_name().and_then(|value| value.to_str());

        if file_name == Some("trainer-sources.json") {
            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(err) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %err,
                        "failed to read trainer-sources.json — skipping"
                    );
                    continue;
                }
            };
            let manifest: TrainerSourcesManifest = match serde_json::from_str(&content) {
                Ok(m) => m,
                Err(err) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %err,
                        "failed to parse trainer-sources.json — skipping"
                    );
                    continue;
                }
            };
            let dir_relative = path
                .parent()
                .unwrap_or(current_dir)
                .strip_prefix(root)
                .unwrap_or_else(|_| path.parent().unwrap_or(current_dir))
                .to_string_lossy()
                .to_string();
            index.trainer_sources.push((dir_relative, manifest));
            continue;
        }

        if file_name != Some("community-profile.json") {
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
                extra: toml::Table::new(),
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
                trainer_sha256: None,
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
                trainer_sha256: None,
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

    #[cfg(unix)]
    mod symlink_skipping {
        use super::*;
        use std::os::unix::fs::symlink;

        fn write_manifest(path: &Path) {
            let manifest = CommunityProfileManifest::new(
                CommunityProfileMetadata {
                    game_name: "Game".to_string(),
                    game_version: String::new(),
                    trainer_name: String::new(),
                    trainer_version: String::new(),
                    proton_version: String::new(),
                    platform_tags: vec![],
                    compatibility_rating: CompatibilityRating::Unknown,
                    author: String::new(),
                    description: String::new(),
                    trainer_sha256: None,
                },
                GameProfile::default(),
            );
            fs::write(path, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
        }

        #[test]
        fn skips_symlinked_manifest_and_directory() {
            let temp_dir = tempdir().unwrap();
            let tap_root = temp_dir.path().join("tap");
            fs::create_dir_all(&tap_root).unwrap();
            // Outside content that must never be indexed through a link.
            let outside = temp_dir.path().join("outside");
            fs::create_dir_all(&outside).unwrap();
            write_manifest(&outside.join("community-profile.json"));
            write_manifest(&tap_root.join("community-profile.json"));
            symlink(&outside, tap_root.join("linked-dir")).unwrap();
            symlink(
                outside.join("community-profile.json"),
                tap_root.join("linked-manifest.json"),
            )
            .unwrap();
            symlink(
                outside.join("community-profile.json"),
                tap_root.join("community-profile-link"),
            )
            .unwrap();

            let index = index_tap(&sample_workspace(tap_root)).unwrap();
            assert_eq!(
                index.entries.len(),
                1,
                "only the real in-tap manifest may be indexed"
            );
            assert!(index.entries[0]
                .manifest_path
                .ends_with("tap/community-profile.json"));
        }

        #[test]
        fn skips_symlink_cycle() {
            let temp_dir = tempdir().unwrap();
            let tap_root = temp_dir.path().join("tap");
            fs::create_dir_all(&tap_root).unwrap();
            write_manifest(&tap_root.join("community-profile.json"));
            symlink(&tap_root, tap_root.join("cycle")).unwrap();

            let index = index_tap(&sample_workspace(tap_root)).unwrap();
            assert_eq!(index.entries.len(), 1);
        }

        #[test]
        fn skips_fifo_named_like_a_manifest() {
            let temp_dir = tempdir().unwrap();
            let tap_root = temp_dir.path().join("tap");
            fs::create_dir_all(&tap_root).unwrap();

            let fifo = tap_root.join("community-profile.json");
            let status = std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .expect("spawn mkfifo");
            assert!(status.success(), "mkfifo failed: {status}");

            // Index on a worker thread: without the regular-file guard the read
            // of the FIFO blocks forever, which must fail the test, not hang it.
            let (sender, receiver) = std::sync::mpsc::channel();
            let workspace = sample_workspace(tap_root);
            std::thread::spawn(move || {
                let _ = sender.send(index_tap(&workspace));
            });
            match receiver.recv_timeout(std::time::Duration::from_secs(10)) {
                Ok(result) => {
                    let index = result.unwrap();
                    assert!(
                        index.entries.is_empty(),
                        "a FIFO named community-profile.json must be skipped, never opened"
                    );
                }
                Err(_) => {
                    // Release the blocked reader (O_RDWR never blocks on Linux)
                    // so the worker thread can finish, then fail.
                    let _ = fs::OpenOptions::new().read(true).write(true).open(&fifo);
                    panic!("indexer blocked reading a FIFO named community-profile.json");
                }
            }
        }
    }
}
