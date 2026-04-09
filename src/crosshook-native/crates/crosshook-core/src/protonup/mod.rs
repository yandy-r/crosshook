//! ProtonUp integration types and service contracts.
//!
//! This module defines the shared DTOs used across catalog retrieval, install
//! execution, and runtime suggestion matching. All types derive `Serialize` /
//! `Deserialize` so they can cross the Tauri IPC boundary.

pub mod catalog;
pub mod install;
pub mod matching;

use serde::{Deserialize, Serialize};

/// Supported ProtonUp providers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProtonUpProvider {
    GeProton,
    ProtonCachyos,
}

impl std::fmt::Display for ProtonUpProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GeProton => write!(f, "ge-proton"),
            Self::ProtonCachyos => write!(f, "proton-cachyos"),
        }
    }
}

/// Parse a provider id from IPC or UI. Unknown values default to GE-Proton for backward compatibility.
pub fn parse_protonup_provider(s: Option<&str>) -> ProtonUpProvider {
    match s.map(str::trim).filter(|x| !x.is_empty()) {
        None | Some("ge-proton") => ProtonUpProvider::GeProton,
        Some("proton-cachyos") => ProtonUpProvider::ProtonCachyos,
        Some(_) => ProtonUpProvider::GeProton,
    }
}

impl Default for ProtonUpProvider {
    fn default() -> Self {
        Self::GeProton
    }
}

/// An available version from a provider catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonUpAvailableVersion {
    pub provider: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_size: Option<u64>,
}

/// Cache freshness metadata attached to catalog responses.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProtonUpCacheMeta {
    pub stale: bool,
    pub offline: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

/// Response for listing available versions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonUpCatalogResponse {
    pub versions: Vec<ProtonUpAvailableVersion>,
    pub cache: ProtonUpCacheMeta,
}

/// Request payload for installing a version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonUpInstallRequest {
    pub provider: String,
    pub version: String,
    pub target_root: String,
    #[serde(default)]
    pub force: bool,
}

/// Categorized install failure reasons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtonUpInstallErrorKind {
    DependencyMissing,
    PermissionDenied,
    ChecksumFailed,
    NetworkError,
    InvalidPath,
    AlreadyInstalled,
    Unknown,
}

impl std::fmt::Display for ProtonUpInstallErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DependencyMissing => write!(f, "dependency_missing"),
            Self::PermissionDenied => write!(f, "permission_denied"),
            Self::ChecksumFailed => write!(f, "checksum_failed"),
            Self::NetworkError => write!(f, "network_error"),
            Self::InvalidPath => write!(f, "invalid_path"),
            Self::AlreadyInstalled => write!(f, "already_installed"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Structured install result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonUpInstallResult {
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<ProtonUpInstallErrorKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

/// Advisory match status for recommendation UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtonUpMatchStatus {
    Matched,
    Missing,
    Unknown,
}

impl Default for ProtonUpMatchStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Suggestion result comparing community requirement to installed runtimes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonUpSuggestion {
    pub status: ProtonUpMatchStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub community_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_install_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ProtonUpProvider ──────────────────────────────────────────────────────

    #[test]
    fn provider_serializes_as_kebab_case() {
        let json =
            serde_json::to_string(&ProtonUpProvider::GeProton).expect("serialize ProtonUpProvider");
        assert_eq!(json, r#""ge-proton""#);
    }

    #[test]
    fn provider_deserializes_from_kebab_case() {
        let provider: ProtonUpProvider =
            serde_json::from_str(r#""ge-proton""#).expect("deserialize ProtonUpProvider");
        assert_eq!(provider, ProtonUpProvider::GeProton);
    }

    #[test]
    fn proton_cachyos_serializes_as_kebab_case() {
        let json = serde_json::to_string(&ProtonUpProvider::ProtonCachyos)
            .expect("serialize ProtonUpProvider");
        assert_eq!(json, r#""proton-cachyos""#);
    }

    #[test]
    fn proton_cachyos_deserializes_from_kebab_case() {
        let provider: ProtonUpProvider =
            serde_json::from_str(r#""proton-cachyos""#).expect("deserialize ProtonUpProvider");
        assert_eq!(provider, ProtonUpProvider::ProtonCachyos);
    }

    #[test]
    fn display_matches_kebab_case_ids() {
        assert_eq!(ProtonUpProvider::GeProton.to_string(), "ge-proton");
        assert_eq!(
            ProtonUpProvider::ProtonCachyos.to_string(),
            "proton-cachyos"
        );
    }

    #[test]
    fn parse_protonup_provider_defaults_and_maps() {
        assert_eq!(parse_protonup_provider(None), ProtonUpProvider::GeProton);
        assert_eq!(
            parse_protonup_provider(Some("")),
            ProtonUpProvider::GeProton
        );
        assert_eq!(
            parse_protonup_provider(Some("ge-proton")),
            ProtonUpProvider::GeProton
        );
        assert_eq!(
            parse_protonup_provider(Some("proton-cachyos")),
            ProtonUpProvider::ProtonCachyos
        );
        assert_eq!(
            parse_protonup_provider(Some("unknown")),
            ProtonUpProvider::GeProton
        );
    }

    // ── ProtonUpMatchStatus ───────────────────────────────────────────────────

    #[test]
    fn match_status_matched_serializes_as_snake_case() {
        let json = serde_json::to_string(&ProtonUpMatchStatus::Matched).expect("serialize Matched");
        assert_eq!(json, r#""matched""#);
    }

    #[test]
    fn match_status_missing_serializes_as_snake_case() {
        let json = serde_json::to_string(&ProtonUpMatchStatus::Missing).expect("serialize Missing");
        assert_eq!(json, r#""missing""#);
    }

    #[test]
    fn match_status_unknown_serializes_as_snake_case() {
        let json = serde_json::to_string(&ProtonUpMatchStatus::Unknown).expect("serialize Unknown");
        assert_eq!(json, r#""unknown""#);
    }

    // ── ProtonUpInstallErrorKind ──────────────────────────────────────────────

    #[test]
    fn install_error_kind_serializes_as_snake_case() {
        let cases = [
            (
                ProtonUpInstallErrorKind::DependencyMissing,
                "dependency_missing",
            ),
            (
                ProtonUpInstallErrorKind::PermissionDenied,
                "permission_denied",
            ),
            (ProtonUpInstallErrorKind::ChecksumFailed, "checksum_failed"),
            (ProtonUpInstallErrorKind::NetworkError, "network_error"),
            (ProtonUpInstallErrorKind::InvalidPath, "invalid_path"),
            (
                ProtonUpInstallErrorKind::AlreadyInstalled,
                "already_installed",
            ),
            (ProtonUpInstallErrorKind::Unknown, "unknown"),
        ];

        for (kind, expected) in cases {
            let json = serde_json::to_string(&kind).unwrap_or_else(|_| panic!("serialize {kind}"));
            assert_eq!(json, format!(r#""{expected}""#), "mismatch for {kind}");
        }
    }

    #[test]
    fn install_error_kind_deserializes_from_snake_case() {
        let kind: ProtonUpInstallErrorKind =
            serde_json::from_str(r#""checksum_failed""#).expect("deserialize ChecksumFailed");
        assert_eq!(kind, ProtonUpInstallErrorKind::ChecksumFailed);
    }

    // ── ProtonUpCatalogResponse round-trip ────────────────────────────────────

    #[test]
    fn catalog_response_round_trip() {
        let original = ProtonUpCatalogResponse {
            versions: vec![
                ProtonUpAvailableVersion {
                    provider: "ge-proton".to_string(),
                    version: "GE-Proton9-21".to_string(),
                    release_url: Some("https://example.com/release".to_string()),
                    download_url: Some("https://example.com/file.tar.gz".to_string()),
                    checksum_url: Some("https://example.com/file.sha512sum".to_string()),
                    checksum_kind: Some("sha512".to_string()),
                    asset_size: Some(123_456),
                },
                ProtonUpAvailableVersion {
                    provider: "ge-proton".to_string(),
                    version: "GE-Proton9-20".to_string(),
                    release_url: None,
                    download_url: None,
                    checksum_url: None,
                    checksum_kind: None,
                    asset_size: None,
                },
            ],
            cache: ProtonUpCacheMeta {
                stale: false,
                offline: false,
                fetched_at: Some("2024-01-01T00:00:00Z".to_string()),
                expires_at: Some("2024-01-01T06:00:00Z".to_string()),
            },
        };

        let json = serde_json::to_string(&original).expect("serialize catalog response");
        let deserialized: ProtonUpCatalogResponse =
            serde_json::from_str(&json).expect("deserialize catalog response");

        assert_eq!(deserialized.versions.len(), 2);
        assert_eq!(deserialized.versions[0].version, "GE-Proton9-21");
        assert_eq!(deserialized.versions[1].version, "GE-Proton9-20");
        assert!(!deserialized.cache.stale);
        assert!(!deserialized.cache.offline);
        assert_eq!(
            deserialized.cache.fetched_at.as_deref(),
            Some("2024-01-01T00:00:00Z")
        );
    }

    #[test]
    fn catalog_response_optional_fields_omitted_when_none() {
        let version = ProtonUpAvailableVersion {
            provider: "ge-proton".to_string(),
            version: "GE-Proton9-20".to_string(),
            release_url: None,
            download_url: None,
            checksum_url: None,
            checksum_kind: None,
            asset_size: None,
        };

        let json = serde_json::to_string(&version).expect("serialize version");
        // None fields should be absent entirely, not serialized as null.
        assert!(!json.contains("release_url"));
        assert!(!json.contains("download_url"));
        assert!(!json.contains("checksum_url"));
        assert!(!json.contains("asset_size"));
    }

    // ── ProtonUpInstallResult round-trip ──────────────────────────────────────

    #[test]
    fn install_result_success_round_trip() {
        let original = ProtonUpInstallResult {
            success: true,
            installed_path: Some(
                "/home/user/.steam/root/compatibilitytools.d/GE-Proton9-21".to_string(),
            ),
            error_kind: None,
            error_message: None,
        };

        let json = serde_json::to_string(&original).expect("serialize install result");
        let deserialized: ProtonUpInstallResult =
            serde_json::from_str(&json).expect("deserialize install result");

        assert!(deserialized.success);
        assert!(deserialized.installed_path.is_some());
        assert!(deserialized.error_kind.is_none());
        assert!(deserialized.error_message.is_none());
    }

    #[test]
    fn install_result_failure_round_trip() {
        let original = ProtonUpInstallResult {
            success: false,
            installed_path: None,
            error_kind: Some(ProtonUpInstallErrorKind::ChecksumFailed),
            error_message: Some("SHA-512 mismatch".to_string()),
        };

        let json = serde_json::to_string(&original).expect("serialize install result");
        let deserialized: ProtonUpInstallResult =
            serde_json::from_str(&json).expect("deserialize install result");

        assert!(!deserialized.success);
        assert!(deserialized.installed_path.is_none());
        assert_eq!(
            deserialized.error_kind,
            Some(ProtonUpInstallErrorKind::ChecksumFailed)
        );
        assert_eq!(
            deserialized.error_message.as_deref(),
            Some("SHA-512 mismatch")
        );
    }

    // ── ProtonUpSuggestion round-trip ─────────────────────────────────────────

    #[test]
    fn suggestion_matched_round_trip() {
        let original = ProtonUpSuggestion {
            status: ProtonUpMatchStatus::Matched,
            community_version: Some("GE-Proton9-21".to_string()),
            matched_install_name: Some("GE-Proton9-21".to_string()),
            recommended_version: None,
        };

        let json = serde_json::to_string(&original).expect("serialize suggestion");
        let deserialized: ProtonUpSuggestion =
            serde_json::from_str(&json).expect("deserialize suggestion");

        assert_eq!(deserialized.status, ProtonUpMatchStatus::Matched);
        assert_eq!(
            deserialized.community_version.as_deref(),
            Some("GE-Proton9-21")
        );
        assert_eq!(
            deserialized.matched_install_name.as_deref(),
            Some("GE-Proton9-21")
        );
        assert!(deserialized.recommended_version.is_none());
    }

    #[test]
    fn suggestion_missing_round_trip() {
        let original = ProtonUpSuggestion {
            status: ProtonUpMatchStatus::Missing,
            community_version: Some("GE-Proton9-21".to_string()),
            matched_install_name: None,
            recommended_version: Some("GE-Proton9-21".to_string()),
        };

        let json = serde_json::to_string(&original).expect("serialize suggestion");
        let deserialized: ProtonUpSuggestion =
            serde_json::from_str(&json).expect("deserialize suggestion");

        assert_eq!(deserialized.status, ProtonUpMatchStatus::Missing);
        assert!(deserialized.matched_install_name.is_none());
        assert_eq!(
            deserialized.recommended_version.as_deref(),
            Some("GE-Proton9-21")
        );
    }

    #[test]
    fn suggestion_unknown_round_trip() {
        let original = ProtonUpSuggestion {
            status: ProtonUpMatchStatus::Unknown,
            community_version: None,
            matched_install_name: None,
            recommended_version: None,
        };

        let json = serde_json::to_string(&original).expect("serialize suggestion");
        let deserialized: ProtonUpSuggestion =
            serde_json::from_str(&json).expect("deserialize suggestion");

        assert_eq!(deserialized.status, ProtonUpMatchStatus::Unknown);
        assert!(deserialized.community_version.is_none());
    }
}
