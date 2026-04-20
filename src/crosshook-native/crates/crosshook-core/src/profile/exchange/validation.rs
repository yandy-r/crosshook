use super::super::community_schema::COMMUNITY_PROFILE_SCHEMA_VERSION;
use super::error::CommunityExchangeError;
use serde_json::Value;

pub fn validate_manifest_value(value: &Value) -> Result<(), CommunityExchangeError> {
    let root = value
        .as_object()
        .ok_or_else(|| CommunityExchangeError::InvalidManifest {
            message: "manifest must be a JSON object".to_string(),
        })?;

    let schema_version = root
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| CommunityExchangeError::InvalidManifest {
            message: "manifest must include an integer schema_version".to_string(),
        })? as u32;
    validate_schema_version(schema_version)?;

    let metadata = required_object(root, "metadata")?;
    for field in [
        "game_name",
        "game_version",
        "trainer_name",
        "trainer_version",
        "proton_version",
        "platform_tags",
        "compatibility_rating",
        "author",
        "description",
    ] {
        require_field(metadata, field)?;
    }

    let profile = required_object(root, "profile")?;
    for field in ["game", "trainer", "injection", "steam", "launch"] {
        require_field(profile, field)?;
    }

    Ok(())
}

pub fn validate_schema_version(version: u32) -> Result<(), CommunityExchangeError> {
    if version > COMMUNITY_PROFILE_SCHEMA_VERSION {
        return Err(CommunityExchangeError::UnsupportedSchemaVersion {
            version,
            supported: COMMUNITY_PROFILE_SCHEMA_VERSION,
        });
    }

    Ok(())
}

fn required_object<'a>(
    parent: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<&'a serde_json::Map<String, Value>, CommunityExchangeError> {
    parent.get(field).and_then(Value::as_object).ok_or_else(|| {
        CommunityExchangeError::InvalidManifest {
            message: format!("manifest field '{field}' must be a JSON object"),
        }
    })
}

fn require_field(
    parent: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<(), CommunityExchangeError> {
    if parent.contains_key(field) {
        Ok(())
    } else {
        Err(CommunityExchangeError::InvalidManifest {
            message: format!("manifest is missing required field '{field}'"),
        })
    }
}
