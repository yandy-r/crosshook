use std::fmt;
use std::sync::OnceLock;
use std::time::Duration;

use reqwest::redirect::Policy;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

const UMU_API_URL: &str = "https://umu.openwinecomponents.org/umu_api.php";
const REQUEST_TIMEOUT_SECS: u64 = 2;
const MAX_UMU_ID_LEN: usize = 128;
const MAX_UMU_API_BODY_BYTES: u64 = 256 * 1024;

static UMU_API_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn api_url() -> String {
    #[cfg(any(test, debug_assertions))]
    {
        if let Ok(url) = std::env::var("CROSSHOOK_TEST_UMU_GAMEID_API_URL") {
            let trimmed = url.trim();
            if !trimmed.is_empty() && test_api_url_allowed(trimmed) {
                return trimmed.to_string();
            }
        }
    }
    UMU_API_URL.to_string()
}

#[cfg(any(test, debug_assertions))]
fn test_api_url_allowed(url: &str) -> bool {
    if std::env::var("CROSSHOOK_TEST_ALLOW_REMOTE_HTTP").is_ok_and(|value| value == "1") {
        return true;
    }

    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    matches!(
        parsed.host_str(),
        Some("localhost") | Some("127.0.0.1") | Some("::1") | Some("[::1]")
    )
}

#[derive(Debug)]
pub enum UmuGameIdApiError {
    InvalidUrl(String),
    Network(reqwest::Error),
    HttpStatus(StatusCode),
    ResponseTooLarge { limit: u64 },
    InvalidResponse(String),
}

impl fmt::Display for UmuGameIdApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUrl(error) => write!(f, "invalid umu GAMEID API URL: {error}"),
            Self::Network(error) => write!(f, "umu GAMEID API network error: {error}"),
            Self::HttpStatus(status) => write!(f, "umu GAMEID API returned HTTP {status}"),
            Self::ResponseTooLarge { limit } => {
                write!(f, "umu GAMEID API response exceeded {limit} bytes")
            }
            Self::InvalidResponse(error) => write!(f, "invalid umu GAMEID API response: {error}"),
        }
    }
}

impl std::error::Error for UmuGameIdApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Network(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UmuGameIdApiEntry {
    pub title: Option<String>,
    pub umu_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UmuGameIdApiLookup {
    Found(UmuGameIdApiEntry, String),
    NotFound(String),
}

#[derive(Debug, Deserialize, Serialize)]
struct RawUmuGameIdApiEntry {
    title: Option<String>,
    umu_id: Option<String>,
}

fn umu_api_http_client() -> Result<&'static reqwest::Client, UmuGameIdApiError> {
    if let Some(client) = UMU_API_HTTP_CLIENT.get() {
        return Ok(client);
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .redirect(Policy::limited(0))
        .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(UmuGameIdApiError::Network)?;
    let _ = UMU_API_HTTP_CLIENT.set(client);
    Ok(UMU_API_HTTP_CLIENT
        .get()
        .expect("umu API HTTP client should be initialized"))
}

pub async fn lookup_umu_game_id(
    store: &str,
    codename: &str,
) -> Result<UmuGameIdApiLookup, UmuGameIdApiError> {
    lookup_umu_game_id_with_api_url(&api_url(), store, codename).await
}

async fn lookup_umu_game_id_with_api_url(
    api_url: &str,
    store: &str,
    codename: &str,
) -> Result<UmuGameIdApiLookup, UmuGameIdApiError> {
    let mut url = reqwest::Url::parse(api_url)
        .map_err(|error| UmuGameIdApiError::InvalidUrl(error.to_string()))?;
    url.query_pairs_mut()
        .append_pair("store", store.trim())
        .append_pair("codename", codename.trim());

    let response = umu_api_http_client()?
        .get(url)
        .send()
        .await
        .map_err(UmuGameIdApiError::Network)?;

    let status = response.status();
    if !status.is_success() {
        return Err(UmuGameIdApiError::HttpStatus(status));
    }

    let response_body = read_limited_body(response).await?;
    let raw_entries: Vec<RawUmuGameIdApiEntry> = serde_json::from_slice(&response_body)
        .map_err(|error| UmuGameIdApiError::InvalidResponse(error.to_string()))?;
    let payload_json = serde_json::to_string(&raw_entries).unwrap_or_else(|_| "[]".to_string());

    for raw in raw_entries {
        let Some(umu_id) = raw.umu_id.map(|value| value.trim().to_string()) else {
            continue;
        };
        if validate_umu_id(&umu_id).is_ok() {
            return Ok(UmuGameIdApiLookup::Found(
                UmuGameIdApiEntry {
                    title: raw.title,
                    umu_id,
                },
                payload_json,
            ));
        }
    }

    Ok(UmuGameIdApiLookup::NotFound(payload_json))
}

async fn read_limited_body(mut response: reqwest::Response) -> Result<Vec<u8>, UmuGameIdApiError> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_UMU_API_BODY_BYTES)
    {
        return Err(UmuGameIdApiError::ResponseTooLarge {
            limit: MAX_UMU_API_BODY_BYTES,
        });
    }

    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(UmuGameIdApiError::Network)? {
        let next_len = body.len() as u64 + chunk.len() as u64;
        if next_len > MAX_UMU_API_BODY_BYTES {
            return Err(UmuGameIdApiError::ResponseTooLarge {
                limit: MAX_UMU_API_BODY_BYTES,
            });
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

pub fn validate_umu_id(umu_id: &str) -> Result<(), UmuGameIdApiError> {
    let trimmed = umu_id.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_UMU_ID_LEN {
        return Err(UmuGameIdApiError::InvalidResponse(
            "umu_id length is invalid".to_string(),
        ));
    }
    if !trimmed
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
    {
        return Err(UmuGameIdApiError::InvalidResponse(
            "umu_id contains unsupported characters".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock should not be poisoned")
    }

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            unsafe { std::env::set_var(key, value) };
            Self { key, previous }
        }

        fn remove(key: &'static str) -> Self {
            let previous = std::env::var(key).ok();
            unsafe { std::env::remove_var(key) };
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => unsafe { std::env::set_var(self.key, value) },
                None => unsafe { std::env::remove_var(self.key) },
            }
        }
    }

    #[tokio::test]
    async fn lookup_builds_expected_url_and_returns_first_valid_entry() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/umu_api.php"))
            .and(query_param("store", "steam"))
            .and(query_param("codename", "Game Name"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "title": "Game Name",
                    "umu_id": "umu-123_ok.1"
                }
            ])))
            .mount(&mock_server)
            .await;

        let api_url = format!("{}/umu_api.php", mock_server.uri());
        let result = lookup_umu_game_id_with_api_url(&api_url, " steam ", " Game Name ")
            .await
            .expect("lookup should succeed");

        assert_eq!(
            result,
            UmuGameIdApiLookup::Found(
                UmuGameIdApiEntry {
                    title: Some("Game Name".to_string()),
                    umu_id: "umu-123_ok.1".to_string(),
                },
                r#"[{"title":"Game Name","umu_id":"umu-123_ok.1"}]"#.to_string(),
            )
        );
    }

    #[tokio::test]
    async fn lookup_returns_not_found_for_empty_array() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/umu_api.php"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .mount(&mock_server)
            .await;

        let api_url = format!("{}/umu_api.php", mock_server.uri());
        let result = lookup_umu_game_id_with_api_url(&api_url, "steam", "missing")
            .await
            .expect("empty lookup response should be valid");

        assert_eq!(result, UmuGameIdApiLookup::NotFound("[]".to_string()));
    }

    #[tokio::test]
    async fn lookup_rejects_non_success_status() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/umu_api.php"))
            .respond_with(ResponseTemplate::new(503).set_body_string("unavailable"))
            .mount(&mock_server)
            .await;

        let api_url = format!("{}/umu_api.php", mock_server.uri());
        let result = lookup_umu_game_id_with_api_url(&api_url, "steam", "game").await;

        assert!(
            matches!(
                result,
                Err(UmuGameIdApiError::HttpStatus(
                    StatusCode::SERVICE_UNAVAILABLE
                ))
            ),
            "expected HTTP status error, got {result:?}"
        );
    }

    #[tokio::test]
    async fn lookup_classifies_malformed_json_as_invalid_response() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/umu_api.php"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_string("{not json"),
            )
            .mount(&mock_server)
            .await;

        let api_url = format!("{}/umu_api.php", mock_server.uri());
        let result = lookup_umu_game_id_with_api_url(&api_url, "steam", "game").await;

        assert!(
            matches!(result, Err(UmuGameIdApiError::InvalidResponse(_))),
            "expected InvalidResponse for malformed JSON, got {result:?}"
        );
    }

    #[tokio::test]
    async fn lookup_rejects_redirects() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/umu_api.php"))
            .respond_with(
                ResponseTemplate::new(302).insert_header("location", "https://example.com/"),
            )
            .mount(&mock_server)
            .await;

        let api_url = format!("{}/umu_api.php", mock_server.uri());
        let result = lookup_umu_game_id_with_api_url(&api_url, "steam", "game").await;

        match result {
            Err(UmuGameIdApiError::Network(error)) => {
                assert!(error.is_redirect(), "expected redirect error, got {error}");
            }
            other => panic!("expected redirect to be rejected, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn lookup_rejects_oversized_body() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/umu_api.php"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_bytes(vec![b' '; MAX_UMU_API_BODY_BYTES as usize + 1]),
            )
            .mount(&mock_server)
            .await;

        let api_url = format!("{}/umu_api.php", mock_server.uri());
        let result = lookup_umu_game_id_with_api_url(&api_url, "steam", "game").await;

        assert!(
            matches!(result, Err(UmuGameIdApiError::ResponseTooLarge { .. })),
            "expected response size error, got {result:?}"
        );
    }

    #[tokio::test]
    async fn lookup_classifies_timeout_as_network_timeout() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/umu_api.php"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_secs(REQUEST_TIMEOUT_SECS + 1))
                    .set_body_json(serde_json::json!([])),
            )
            .mount(&mock_server)
            .await;

        let api_url = format!("{}/umu_api.php", mock_server.uri());
        let result = lookup_umu_game_id_with_api_url(&api_url, "steam", "slow").await;

        match result {
            Err(UmuGameIdApiError::Network(error)) => {
                assert!(error.is_timeout(), "expected timeout error, got {error}");
            }
            other => panic!("expected network timeout error, got {other:?}"),
        }
    }

    #[test]
    fn validate_umu_id_accepts_and_rejects_edge_cases() {
        let max_length_id = "a".repeat(MAX_UMU_ID_LEN);
        let too_long_id = "a".repeat(MAX_UMU_ID_LEN + 1);

        for value in ["umu-123_ok.1", "  trimmed_valid  ", &max_length_id] {
            assert!(
                validate_umu_id(value).is_ok(),
                "expected valid umu_id: {value:?}"
            );
        }

        for value in ["", "   ", "bad/id", "bad id", "bad:id", &too_long_id] {
            assert!(
                matches!(
                    validate_umu_id(value),
                    Err(UmuGameIdApiError::InvalidResponse(_))
                ),
                "expected invalid umu_id: {value:?}"
            );
        }
    }

    #[test]
    fn debug_api_url_override_requires_loopback_unless_explicitly_allowed() {
        let _env_lock = lock_env();
        let _api_url = EnvGuard::set(
            "CROSSHOOK_TEST_UMU_GAMEID_API_URL",
            "https://remote.example/umu_api.php",
        );
        let _allow_remote = EnvGuard::remove("CROSSHOOK_TEST_ALLOW_REMOTE_HTTP");

        assert_eq!(api_url(), UMU_API_URL);

        let _allow_remote = EnvGuard::set("CROSSHOOK_TEST_ALLOW_REMOTE_HTTP", "1");
        assert_eq!(api_url(), "https://remote.example/umu_api.php");
    }

    #[test]
    fn debug_api_url_override_allows_loopback_mock_hosts() {
        let _env_lock = lock_env();
        let _allow_remote = EnvGuard::remove("CROSSHOOK_TEST_ALLOW_REMOTE_HTTP");

        for url in [
            "http://localhost:1234/umu_api.php",
            "http://127.0.0.1:1234/umu_api.php",
            "http://[::1]:1234/umu_api.php",
        ] {
            let _api_url = EnvGuard::set("CROSSHOOK_TEST_UMU_GAMEID_API_URL", url);
            assert_eq!(api_url(), url);
        }
    }
}
