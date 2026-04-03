pub const STEAM_METADATA_CACHE_NAMESPACE: &str = "steam:appdetails:v1";

pub fn normalize_app_id(app_id: &str) -> Option<String> {
    let trimmed = app_id.trim();
    if trimmed.is_empty() || !trimmed.chars().all(|c| c.is_ascii_digit()) {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn cache_key_for_app_id(app_id: &str) -> Option<String> {
    normalize_app_id(app_id)
        .map(|normalized| format!("{}:{}", STEAM_METADATA_CACHE_NAMESPACE, normalized))
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SteamMetadataLookupState {
    #[default]
    Idle,
    Loading,
    Ready,
    Stale,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub struct SteamGenre {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub struct SteamAppDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_image: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub genres: Vec<SteamGenre>,
}

/// Top-level lookup result used by the IPC and UI layers.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, Default)]
pub struct SteamMetadataLookupResult {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub app_id: String,
    #[serde(default)]
    pub state: SteamMetadataLookupState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_details: Option<SteamAppDetails>,
    #[serde(default)]
    pub from_cache: bool,
    #[serde(default)]
    pub is_stale: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_app_id_rejects_non_numeric_and_empty() {
        assert_eq!(normalize_app_id("1245620"), Some("1245620".to_string()));
        assert_eq!(normalize_app_id("  1245620  "), Some("1245620".to_string()));
        assert_eq!(normalize_app_id("   "), None);
        assert_eq!(normalize_app_id("abc123"), None);
        assert_eq!(normalize_app_id("12.5"), None);
    }

    #[test]
    fn cache_key_is_namespaced() {
        assert_eq!(
            cache_key_for_app_id("1245620"),
            Some("steam:appdetails:v1:1245620".to_string())
        );
        assert_eq!(
            cache_key_for_app_id("  1245620  "),
            Some("steam:appdetails:v1:1245620".to_string())
        );
        assert_eq!(cache_key_for_app_id("abc123"), None);
    }

    #[test]
    fn lookup_state_serializes_as_snake_case() {
        let json = serde_json::to_string(&SteamMetadataLookupState::Ready).unwrap();
        assert_eq!(json, r#""ready""#);
        let json = serde_json::to_string(&SteamMetadataLookupState::Unavailable).unwrap();
        assert_eq!(json, r#""unavailable""#);
    }
}
