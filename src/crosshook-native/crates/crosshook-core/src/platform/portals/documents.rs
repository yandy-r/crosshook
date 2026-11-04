//! `org.freedesktop.portal.Documents` client.
//!
//! See `docs/architecture/adr-0002-flatpak-portal-contracts.md` § Documents
//! portal contract. Only `GetHostPaths` (interface version 5, shipped with
//! xdg-desktop-portal 1.19) is used: a read-only lookup that never prompts
//! the user. Path parsing and validation live in `platform::picked_path`.

use std::collections::HashMap;
use std::time::Duration;

/// D-Bus destination of the document portal.
const DOCUMENTS_PORTAL_BUS: &str = "org.freedesktop.portal.Documents";
/// Object path of the document portal.
const DOCUMENTS_PORTAL_PATH: &str = "/org/freedesktop/portal/documents";
/// Interface exposing `GetHostPaths` (version 5+).
const DOCUMENTS_PORTAL_INTERFACE: &str = "org.freedesktop.portal.Documents";
/// Upper bound for the whole `GetHostPaths` round trip, so a hung portal
/// cannot freeze a pick.
const DOCUMENTS_PORTAL_TIMEOUT: Duration = Duration::from_secs(2);

/// Client for `org.freedesktop.portal.Documents.GetHostPaths`.
#[async_trait::async_trait]
pub(crate) trait DocumentsPortal: Send + Sync {
    /// Maps document ids to their raw host paths (D-Bus `a{say}`). Ids the
    /// portal does not know are missing from the map. `Err` means the portal
    /// could not be reached, rejected the call, or timed out.
    async fn get_host_paths(&self, doc_ids: &[String]) -> Result<HashMap<String, Vec<u8>>, String>;
}

/// [`DocumentsPortal`] over the session bus, bounded by a 2 s timeout.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ZbusDocumentsPortal;

#[async_trait::async_trait]
impl DocumentsPortal for ZbusDocumentsPortal {
    async fn get_host_paths(&self, doc_ids: &[String]) -> Result<HashMap<String, Vec<u8>>, String> {
        let call = async {
            let connection = zbus::Connection::session()
                .await
                .map_err(|error| error.to_string())?;
            let proxy = zbus::Proxy::new(
                &connection,
                DOCUMENTS_PORTAL_BUS,
                DOCUMENTS_PORTAL_PATH,
                DOCUMENTS_PORTAL_INTERFACE,
            )
            .await
            .map_err(|error| error.to_string())?;
            // GetHostPaths(IN as doc_ids, OUT a{say} host_paths)
            proxy
                .call::<_, _, HashMap<String, Vec<u8>>>("GetHostPaths", &(doc_ids,))
                .await
                .map_err(|error| error.to_string())
        };

        tokio::time::timeout(DOCUMENTS_PORTAL_TIMEOUT, call)
            .await
            .map_err(|_| {
                format!(
                    "GetHostPaths timed out after {}s",
                    DOCUMENTS_PORTAL_TIMEOUT.as_secs()
                )
            })?
    }
}
