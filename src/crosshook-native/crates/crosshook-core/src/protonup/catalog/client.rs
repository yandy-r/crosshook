use std::time::Duration;

use tokio::sync::OnceCell;

const REQUEST_TIMEOUT_SECS: u64 = 10;

static PROTONUP_HTTP_CLIENT: OnceCell<reqwest::Client> = OnceCell::const_new();

pub(crate) async fn protonup_http_client() -> Result<&'static reqwest::Client, reqwest::Error> {
    PROTONUP_HTTP_CLIENT
        .get_or_try_init(|| async {
            reqwest::Client::builder()
                .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
                .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
                .build()
        })
        .await
}
