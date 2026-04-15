use super::{paths, CsvCoverage};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::SystemTime,
};

// Upstream schema verified 2026-04-14 against
// https://raw.githubusercontent.com/Open-Wine-Components/umu-database/main/umu-database.csv
// TITLE,STORE,CODENAME,UMU_ID,COMMON ACRONYM (Optional),NOTE (Optional),EXE_STRINGS (Optional)
// Fields mirror the upstream CSV schema; not all are accessed at runtime.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CsvRow {
    // Retained for schema fidelity/debugging even though lookup currently keys
    // only by STORE + CODENAME.
    #[allow(dead_code)]
    #[serde(rename = "TITLE")]
    pub title: String,
    #[serde(rename = "STORE")]
    pub store: String,
    #[serde(rename = "CODENAME")]
    pub codename: String,
    #[allow(dead_code)]
    #[serde(rename = "UMU_ID")]
    pub umu_id: String,
    #[allow(dead_code)]
    #[serde(rename = "COMMON ACRONYM (Optional)", default)]
    pub common_acronym: String,
    #[allow(dead_code)]
    #[serde(rename = "NOTE (Optional)", default)]
    pub note: String,
    #[allow(dead_code)]
    #[serde(rename = "EXE_STRINGS (Optional)", default)]
    pub exe_strings: String,
}

type Index = HashMap<(String, String), CsvRow>;

struct CacheEntry {
    path: PathBuf,
    mtime: SystemTime,
    /// File size in bytes (with mtime) for cache invalidation on coarse-mtime filesystems.
    size: u64,
    index: Index,
}
static CACHE: OnceLock<Mutex<Option<CacheEntry>>> = OnceLock::new();

pub fn check_coverage(app_id: &str, store: Option<&str>) -> CsvCoverage {
    let app_id = app_id.trim();
    if app_id.is_empty() {
        return CsvCoverage::Unknown;
    }
    let Some(path) = paths::resolve_umu_database_path() else {
        return CsvCoverage::Unknown;
    };
    let store_key = store.unwrap_or("steam").to_ascii_lowercase();
    let mutex = CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (mtime, size) = match fs::metadata(&path) {
        Ok(m) => match m.modified() {
            Ok(t) => (t, m.len()),
            Err(_) => return CsvCoverage::Unknown,
        },
        Err(_) => return CsvCoverage::Unknown,
    };

    let needs_reload = match guard.as_ref() {
        Some(e) => e.path != path || e.mtime != mtime || e.size != size,
        None => true,
    };
    if needs_reload {
        match load_index(&path) {
            Ok(index) => {
                *guard = Some(CacheEntry {
                    path: path.clone(),
                    mtime,
                    size,
                    index,
                });
            }
            Err(err) => {
                tracing::warn!(path = %path.display(), %err, "failed to parse umu-database CSV");
                *guard = None;
                return CsvCoverage::Unknown;
            }
        }
    }

    let Some(entry) = guard.as_ref() else {
        return CsvCoverage::Unknown;
    };
    let found = entry.index.contains_key(&(store_key, app_id.to_string()));
    tracing::debug!(app_id, store = ?store, found, "umu-database coverage lookup");
    if found {
        CsvCoverage::Found
    } else {
        CsvCoverage::Missing
    }
}

fn load_index(path: &Path) -> csv::Result<Index> {
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .has_headers(true)
        .from_path(path)?;
    let mut out = HashMap::new();
    for row in rdr.deserialize::<CsvRow>() {
        let row = row?;
        let key = (
            row.store.trim().to_ascii_lowercase(),
            row.codename.trim().to_string(),
        );
        out.insert(key, row);
    }
    Ok(out)
}

#[cfg(test)]
pub fn clear_cache_for_test() {
    if let Some(mutex) = CACHE.get() {
        *mutex.lock().expect("lock") = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const FIXTURE: &str = "\
TITLE,STORE,CODENAME,UMU_ID,COMMON ACRONYM (Optional),NOTE (Optional),EXE_STRINGS (Optional)
Ghost of Tsushima,steam,546590,umu-546590,GoT,,ghostoftsushima.exe
Resident Evil 4 Remake,steam,2050650,umu-2050650,RE4R,,re4.exe
";

    #[test]
    fn index_contains_fixture_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("umu-database.csv");
        writeln!(std::fs::File::create(&path).unwrap(), "{FIXTURE}").unwrap();
        let index = load_index(&path).unwrap();
        assert!(index.contains_key(&("steam".to_string(), "546590".to_string())));
    }
}
