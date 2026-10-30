//! Harness: a temp prefix with a `system.reg` fixture, a fake tool script
//! (named `fake-tricks`) that records its argv, in-memory metadata with an
//! indexed profile, and argument-asserting fakes for every seam. Gating uses
//! fifos, never timing sleeps; nothing touches process environment. The
//! seam fakes live in `fakes.rs`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crosshook_core::metadata::{MetadataStore, PrefixVersionRestoreJournalRow, SyncSource};
use crosshook_core::profile::{GameProfile, ProfileStore};
use crosshook_core::settings::SettingsStore;
use tempfile::TempDir;

use super::super::journal::PrefixVersionJournal;
use super::super::seams::{
    CoreRestorer, JournalStore, MetadataJournalStore, PrefixInUse, PrefixVersionRestorer, ToolProbe,
};
use super::super::PrefixDepsService;
use super::fakes::{CountingRestorer, FakeInUse, FakeProbe, RecordingStore};
use crate::jobs::JobRegistry;

/// Name of the indexed profile every harness creates.
pub(crate) const PROFILE: &str = "game";

/// Windows versions the `system.reg` fixture can carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WinVersion {
    Win7,
    Win10,
}

impl WinVersion {
    fn registry_body(self) -> &'static str {
        match self {
            Self::Win7 => {
                "\"CurrentBuild\"=\"7601\"\n\"CurrentVersion\"=\"6.1\"\n\"ProductName\"=\"Microsoft Windows 7\""
            }
            Self::Win10 => {
                "\"CurrentBuild\"=\"19045\"\n\"CurrentVersion\"=\"6.3\"\n\"ProductName\"=\"Microsoft Windows 10\""
            }
        }
    }
}

/// Writes a `system.reg` reporting `version` into `pfx`.
pub(crate) fn write_system_reg(pfx: &Path, version: WinVersion) {
    fs::create_dir_all(pfx).unwrap();
    fs::write(pfx.join("system.reg"), registry(version)).unwrap();
}

fn registry(version: WinVersion) -> String {
    format!(
        "WINE REGISTRY Version 2\n\n[Software\\\\Microsoft\\\\Windows NT\\\\CurrentVersion] 123\n{}\n\n[System\\\\CurrentControlSet] 124\n",
        version.registry_body()
    )
}

/// Behaviour of the fake tool script.
#[derive(Debug, Clone)]
pub(crate) struct ToolScript {
    /// Exit code of an install (any call whose last arg is not a version verb).
    pub install_exit: i32,
    /// Exit code of a `-q win7|win10` restore.
    pub restore_exit: i32,
    /// Whether a restore rewrites `system.reg` to the requested version.
    pub rewrite_reg_on_restore: bool,
    /// When set, an install blocks reading this fifo until it is written.
    pub install_gate: Option<PathBuf>,
}

impl Default for ToolScript {
    fn default() -> Self {
        Self {
            install_exit: 0,
            restore_exit: 0,
            rewrite_reg_on_restore: true,
            install_gate: None,
        }
    }
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}

/// Writes the executable `fake-tricks` script into `dir`; every call appends
/// `"$*"` to `calls`. A restore prints a `/home/…` path to stderr so error
/// sanitization is observable; an install prints one stdout and one stderr line.
pub(crate) fn fake_tool(dir: &Path, calls: &Path, script: &ToolScript) -> String {
    let (win7, win10) = if script.rewrite_reg_on_restore {
        (
            format!(
                "printf '%s' {} > \"$WINEPREFIX/system.reg\"",
                sh_literal(&registry(WinVersion::Win7))
            ),
            format!(
                "printf '%s' {} > \"$WINEPREFIX/system.reg\"",
                sh_literal(&registry(WinVersion::Win10))
            ),
        )
    } else {
        (":".to_string(), ":".to_string())
    };
    let gate = script
        .install_gate
        .as_deref()
        .map(|fifo| format!("cat {} > /dev/null", shell_quote(fifo)))
        .unwrap_or_else(|| ":".to_string());
    let body = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> {calls}\nlast=''\nfor arg in \"$@\"; do last=\"$arg\"; done\ncase \"$last\" in\n  win7) {win7}; echo 'restore error at /home/someone/secret' >&2; exit {restore};;\n  win10) {win10}; echo 'restore error at /home/someone/secret' >&2; exit {restore};;\nesac\necho \"installing $last\"\necho \"install warning $last\" >&2\n{gate}\nexit {install}\n",
        calls = shell_quote(calls),
        restore = script.restore_exit,
        install = script.install_exit,
    );
    let path = dir.join("fake-tricks");
    fs::write(&path, body).unwrap();
    set_mode(&path, 0o755);
    path.to_string_lossy().into_owned()
}

fn sh_literal(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// Sets unix permission bits on `path`.
pub(crate) fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

/// Lines the fake tool recorded; empty when it never ran.
pub(crate) fn read_calls(calls: &Path) -> Vec<String> {
    fs::read_to_string(calls)
        .map(|text| text.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

/// A `pending` journal row for `prefix` restoring `win10` with winetricks.
pub(crate) fn pending_row(prefix: &str, binary: &str) -> PrefixVersionRestoreJournalRow {
    PrefixVersionRestoreJournalRow {
        prefix_path: prefix.to_string(),
        profile_id: None,
        binary_path: binary.to_string(),
        tool_type: "winetricks".to_string(),
        steam_app_id: None,
        restore_verb: "win10".to_string(),
        state: "pending".to_string(),
        last_error: None,
        created_at: "2026-08-24T00:00:00Z".to_string(),
        updated_at: "2026-08-24T00:00:00Z".to_string(),
    }
}

/// Temp dir, canonical prefix (`<dir>/prefix/pfx`, `system.reg` = win7),
/// recorded-calls path, in-memory metadata and file stores with [`PROFILE`].
pub(crate) struct Harness {
    pub dir: TempDir,
    /// Canonical `pfx` directory (the journal and job key path).
    pub prefix: String,
    /// The user-facing prefix root (`<dir>/prefix`).
    pub prefix_root: PathBuf,
    pub calls: PathBuf,
    pub metadata: MetadataStore,
    pub settings: SettingsStore,
    pub profiles: ProfileStore,
    pub profile_id: String,
}

impl Harness {
    pub(crate) fn new() -> Self {
        Self::with_profile(GameProfile::default())
    }

    /// Like [`Harness::new`] with `profile` saved and indexed as [`PROFILE`].
    pub(crate) fn with_profile(profile: GameProfile) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let prefix_root = dir.path().join("prefix");
        let pfx = prefix_root.join("pfx");
        write_system_reg(&pfx, WinVersion::Win7);
        let prefix = fs::canonicalize(&pfx)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let profiles = ProfileStore::with_base_path(dir.path().join("profiles"));
        profiles.save(PROFILE, &profile).unwrap();
        let metadata = MetadataStore::open_in_memory().unwrap();
        metadata
            .observe_profile_write(
                PROFILE,
                &profile,
                &dir.path().join("profiles/game.toml"),
                SyncSource::AppWrite,
                None,
            )
            .unwrap();
        let profile_id = metadata.lookup_profile_id(PROFILE).unwrap().unwrap();
        Self {
            settings: SettingsStore::with_base_path(dir.path().join("settings")),
            calls: dir.path().join("calls.txt"),
            prefix,
            prefix_root,
            metadata,
            profiles,
            profile_id,
            dir,
        }
    }

    /// The canonical prefix as a path.
    pub(crate) fn pfx(&self) -> &Path {
        Path::new(&self.prefix)
    }

    /// Writes the fake tool into the temp dir.
    pub(crate) fn tool(&self, script: &ToolScript) -> String {
        fake_tool(self.dir.path(), &self.calls, script)
    }

    /// Inserts `row` into the real journal table.
    pub(crate) fn seed_row(&self, row: &PrefixVersionRestoreJournalRow) {
        self.metadata.upsert_prefix_version_restore(row).unwrap();
    }

    /// Marks the seeded row `failed` with `error`.
    pub(crate) fn fail_row(&self, error: &str) {
        self.metadata
            .mark_prefix_version_restore_failed(&self.prefix, error)
            .unwrap();
    }

    /// The real journal row of the prefix.
    pub(crate) fn row(&self) -> Option<PrefixVersionRestoreJournalRow> {
        self.metadata
            .load_prefix_version_restore(&self.prefix)
            .unwrap()
    }

    /// A store over the real journal that asserts it is only asked about
    /// this prefix.
    pub(crate) fn recording_store(&self) -> Arc<RecordingStore> {
        Arc::new(RecordingStore::new(self.metadata.clone(), &self.prefix))
    }

    /// A counting real restorer that asserts its plan targets this prefix.
    pub(crate) fn restorer(&self) -> Arc<CountingRestorer> {
        Arc::new(CountingRestorer::new(Arc::new(CoreRestorer), &self.prefix))
    }

    /// Journal over the real store, the real restorer and `in_use`.
    pub(crate) fn journal(&self, in_use: bool) -> (PrefixVersionJournal, Arc<CountingRestorer>) {
        let restorer = self.restorer();
        let journal = PrefixVersionJournal::new(
            Arc::new(MetadataJournalStore(self.metadata.clone())),
            restorer.clone(),
            Arc::new(FakeInUse::new(in_use, &self.prefix)),
        );
        (journal, restorer)
    }

    /// Service whose tool detection finds `tool` as winetricks, over the
    /// real journal store and restorer and a prefix nobody uses.
    pub(crate) fn service(&self, jobs: JobRegistry, tool: &str) -> PrefixDepsService {
        self.service_with(
            jobs,
            Arc::new(MetadataJournalStore(self.metadata.clone())),
            self.restorer(),
            Arc::new(FakeInUse::new(false, &self.prefix)),
            Arc::new(FakeProbe::winetricks(tool)),
        )
    }

    /// Service with explicit seams.
    pub(crate) fn service_with(
        &self,
        jobs: JobRegistry,
        store: Arc<dyn JournalStore>,
        restorer: Arc<dyn PrefixVersionRestorer>,
        in_use: Arc<dyn PrefixInUse>,
        probe: Arc<dyn ToolProbe>,
    ) -> PrefixDepsService {
        PrefixDepsService::with_seams(
            self.settings.clone(),
            self.profiles.clone(),
            self.metadata.clone(),
            jobs,
            store,
            restorer,
            in_use,
            probe,
            self.dir.path().join("logs"),
        )
    }
}
