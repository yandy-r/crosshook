//! HMAC-SHA256 tamper evidence for `config_revisions` (ADR-0005).
//!
//! The key file ([`KEY_FILE_NAME`], `0600`, next to `metadata.db`) holds a
//! 32-byte CSPRNG key followed by a u64 big-endian *signing epoch*: the highest
//! `config_revisions.id` that existed when the key was created. Every revision
//! with `id > epoch` was written under this key and must carry a valid
//! signature; only rows at or below the epoch may fall back to hash-only
//! verification. The key file never enters SQLite, logs, or IPC.
//!
//! Stored signatures are `v1:<key_id>:<trust>:<hex_mac>`. `key_id` is a short
//! SHA-256 fingerprint of the key (not secret-revealing). `trust` is
//! `verified`, or `hash_only` for a revision derived (rollback) from a source
//! that only passed hash verification, so forged legacy content is never
//! upgraded to fully signed. The MAC binds the row id, profile id, source,
//! creation time, content hash, snapshot, and trust.

use std::fmt::{self, Debug, Formatter, Write as _};
use std::fs::{self, File, OpenOptions, Permissions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use super::models::{
    ConfigRevisionRow, HashOnlyReason, IntegrityFailure, MetadataStoreError, RevisionIntegrity,
};
use super::sha256_hex;

/// Key file name, created in the same directory as the metadata database.
pub const KEY_FILE_NAME: &str = "config_history_hmac.key";

const KEY_LEN: usize = 32;
const EPOCH_LEN: usize = 8;
const KEY_FILE_LEN: usize = KEY_LEN + EPOCH_LEN;
const MAC_LEN: usize = 32;
/// Bytes of SHA-256(key) used as the public key fingerprint (16 hex chars).
const KEY_ID_LEN: usize = 8;
const SIGNATURE_VERSION: &str = "v1";
const MESSAGE_TAG: &[u8] = b"crosshook.config_revision.v1";
const TEMP_PREFIX: &str = ".config_history_hmac.key.";
/// Crash leftovers older than this are removed on writable open.
const STALE_TEMP_AGE: Duration = Duration::from_secs(60 * 60);

type HmacSha256 = Hmac<Sha256>;

/// Machine-local config-history signing key. `Debug` prints only the key id
/// and epoch; the key bytes are wiped on drop.
pub(crate) struct ConfigHistoryKey {
    bytes: Zeroizing<[u8; KEY_LEN]>,
    id: String,
    signed_from: i64,
}

/// Trust level carried (and authenticated) inside a signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Trust {
    /// Content authored by CrossHook from a live profile save.
    Verified,
    /// Derived from a revision that only passed hash verification.
    HashOnly,
}

impl Trust {
    fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::HashOnly => "hash_only",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "verified" => Some(Self::Verified),
            "hash_only" => Some(Self::HashOnly),
            _ => None,
        }
    }
}

/// Row fields bound by the MAC.
pub(crate) struct RevisionFields<'a> {
    pub id: i64,
    pub profile_id: &'a str,
    pub source: &'a str,
    pub created_at: &'a str,
    pub content_hash: &'a str,
    pub snapshot_toml: &'a str,
}

impl<'a> RevisionFields<'a> {
    pub(crate) fn from_row(row: &'a ConfigRevisionRow) -> Self {
        Self {
            id: row.id,
            profile_id: &row.profile_id,
            source: &row.source,
            created_at: &row.created_at,
            content_hash: &row.content_hash,
            snapshot_toml: &row.snapshot_toml,
        }
    }
}

/// Result of checking a stored signature against the current key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SignatureCheck {
    Valid(Trust),
    /// Same key id, but the MAC, trust, or encoding does not verify.
    Mismatch,
    /// Unknown version prefix or unparseable layout.
    UnknownFormat,
    /// Signed under a different key.
    ForeignKey,
}

impl Debug for ConfigHistoryKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigHistoryKey")
            .field("id", &self.id)
            .field("signed_from", &self.signed_from)
            .finish_non_exhaustive()
    }
}

impl ConfigHistoryKey {
    fn new(bytes: Zeroizing<[u8; KEY_LEN]>, signed_from: i64) -> Self {
        let digest = Sha256::digest(&bytes[..]);
        Self {
            id: hex_encode(&digest[..KEY_ID_LEN]),
            bytes,
            signed_from,
        }
    }

    /// Public, non-secret fingerprint embedded in signatures.
    #[cfg(test)]
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    /// Highest revision id that predates this key (signing epoch).
    #[cfg(test)]
    pub(crate) fn signed_from(&self) -> i64 {
        self.signed_from
    }

    fn mac(&self, fields: &RevisionFields<'_>, trust: Trust) -> HmacSha256 {
        let mut mac =
            HmacSha256::new_from_slice(&self.bytes[..]).expect("HMAC accepts keys of any length");
        let id = fields.id.to_be_bytes();
        for field in [
            MESSAGE_TAG,
            &id[..],
            fields.profile_id.as_bytes(),
            fields.source.as_bytes(),
            fields.created_at.as_bytes(),
            fields.content_hash.as_bytes(),
            fields.snapshot_toml.as_bytes(),
            trust.as_str().as_bytes(),
        ] {
            mac.update(&(field.len() as u64).to_be_bytes());
            mac.update(field);
        }
        mac
    }

    /// Signs a revision, returning the stored `v1:<key_id>:<trust>:<hex_mac>` form.
    pub(crate) fn sign(&self, fields: &RevisionFields<'_>, trust: Trust) -> String {
        let tag = self.mac(fields, trust).finalize().into_bytes();
        format!(
            "{SIGNATURE_VERSION}:{}:{}:{}",
            self.id,
            trust.as_str(),
            hex_encode(&tag)
        )
    }

    /// Checks a stored signature; the MAC comparison is constant time.
    fn check(&self, signature: &str, fields: &RevisionFields<'_>) -> SignatureCheck {
        let mut parts = signature.splitn(4, ':');
        let (Some(version), Some(key_id), Some(trust), Some(hex_mac)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return SignatureCheck::UnknownFormat;
        };
        if version != SIGNATURE_VERSION {
            return SignatureCheck::UnknownFormat;
        }
        if key_id != self.id {
            return SignatureCheck::ForeignKey;
        }
        // Same key: anything that does not verify is tampering.
        let (Some(trust), Some(tag)) = (Trust::parse(trust), hex_decode_mac(hex_mac)) else {
            return SignatureCheck::Mismatch;
        };
        match self.mac(fields, trust).verify_slice(&tag[..]) {
            Ok(()) => SignatureCheck::Valid(trust),
            Err(_) => SignatureCheck::Mismatch,
        }
    }
}

/// Verifies a revision's content hash and, when `key` is present, its
/// signature under the epoch rules described in the module docs. Pure: the
/// caller decides how to log the outcome.
pub(crate) fn verify_row(
    key: Option<&ConfigHistoryKey>,
    row: &ConfigRevisionRow,
) -> Result<RevisionIntegrity, MetadataStoreError> {
    let fail = |failure| {
        Err(MetadataStoreError::IntegrityCheckFailed {
            revision_id: row.id,
            failure,
        })
    };
    if sha256_hex(row.snapshot_toml.as_bytes()) != row.content_hash {
        return fail(IntegrityFailure::ContentHashMismatch);
    }
    let Some(key) = key else {
        let reason = if row.hmac_signature.is_some() {
            HashOnlyReason::KeyUnavailable
        } else {
            HashOnlyReason::Unsigned
        };
        return Ok(RevisionIntegrity::HashOnly { reason });
    };
    let requires_signature = row.id > key.signed_from;
    let Some(signature) = row.hmac_signature.as_deref() else {
        return if requires_signature {
            fail(IntegrityFailure::SignatureRequired)
        } else {
            Ok(RevisionIntegrity::HashOnly {
                reason: HashOnlyReason::Unsigned,
            })
        };
    };
    match key.check(signature, &RevisionFields::from_row(row)) {
        SignatureCheck::Valid(Trust::Verified) => Ok(RevisionIntegrity::Signed),
        SignatureCheck::Valid(Trust::HashOnly) => Ok(RevisionIntegrity::HashOnly {
            reason: HashOnlyReason::DerivedFromHashOnly,
        }),
        SignatureCheck::ForeignKey if !requires_signature => Ok(RevisionIntegrity::HashOnly {
            reason: HashOnlyReason::ForeignKey,
        }),
        SignatureCheck::ForeignKey | SignatureCheck::UnknownFormat => {
            fail(IntegrityFailure::SignatureRequired)
        }
        SignatureCheck::Mismatch => fail(IntegrityFailure::SignatureMismatch),
    }
}

/// Trust for a new revision: `Verified` for live saves; for a derived
/// (rollback) revision, `Verified` only when the source row itself verifies
/// as fully signed under `key`.
pub(crate) fn derived_trust(key: &ConfigHistoryKey, source: Option<&ConfigRevisionRow>) -> Trust {
    match source.map(|row| verify_row(Some(key), row)) {
        None | Some(Ok(RevisionIntegrity::Signed)) => Trust::Verified,
        Some(_) => Trust::HashOnly,
    }
}

/// Path of the key file for the database at `db_path`.
pub(crate) fn key_path(db_path: &Path) -> PathBuf {
    db_path.with_file_name(KEY_FILE_NAME)
}

/// Loads the key for a file-backed store. Writable stores remove stale temp
/// files and create the key (with `epoch()` as its signing epoch) when absent;
/// read-only stores only load. Failures are logged (never key material) and
/// leave the store on the unsigned / hash-only path.
pub(super) fn open_for_store(
    db_path: &Path,
    writable: bool,
    epoch: impl FnOnce() -> io::Result<i64>,
) -> Option<Arc<ConfigHistoryKey>> {
    let path = key_path(db_path);
    if writable {
        remove_stale_temp_files(&path);
    }
    let result = if writable {
        load_or_create(&path, epoch)
    } else {
        load(&path)
    };
    match result {
        Ok(key) => Some(Arc::new(key)),
        Err(error) if !writable && error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => {
            tracing::warn!(
                %error,
                "config history HMAC key unavailable; new revisions will be unsigned and rollback verifies hash only"
            );
            None
        }
    }
}

/// Loads an existing key, or creates one when the file does not exist.
pub(crate) fn load_or_create(
    path: &Path,
    epoch: impl FnOnce() -> io::Result<i64>,
) -> io::Result<ConfigHistoryKey> {
    match load(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => create_or_adopt(path, epoch()?),
        result => result,
    }
}

/// Opens a regular, non-symlink file owned by the current user, tightening
/// group/other permission bits on the open handle.
pub(crate) fn open_private(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "config history key is not a regular file",
        ));
    }
    if meta.uid() != nix::unistd::geteuid().as_raw() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "config history key is not owned by the current user",
        ));
    }
    if meta.mode() & 0o077 != 0 {
        tracing::warn!("config history HMAC key had group/other permissions; resetting to 0600");
        file.set_permissions(Permissions::from_mode(0o600))?;
    }
    Ok(file)
}

/// Reads exactly [`KEY_FILE_LEN`] bytes (key + epoch) from the key file.
pub(crate) fn load(path: &Path) -> io::Result<ConfigHistoryKey> {
    let mut file = open_private(path)?;
    let mut buf = Zeroizing::new([0_u8; KEY_FILE_LEN + 1]);
    let mut len = 0;
    while len < buf.len() {
        match file.read(&mut buf[len..])? {
            0 => break,
            n => len += n,
        }
    }
    if len != KEY_FILE_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "config history key has an invalid length",
        ));
    }
    let mut bytes = Zeroizing::new([0_u8; KEY_LEN]);
    bytes.copy_from_slice(&buf[..KEY_LEN]);
    let mut epoch = [0_u8; EPOCH_LEN];
    epoch.copy_from_slice(&buf[KEY_LEN..KEY_FILE_LEN]);
    let signed_from = i64::try_from(u64::from_be_bytes(epoch)).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "config history key has an invalid signing epoch",
        )
    })?;
    Ok(ConfigHistoryKey::new(bytes, signed_from))
}

/// Generates a key and publishes it without ever replacing an existing file:
/// key + epoch are written and fsynced to a private temp file, then linked into
/// place with no-replace semantics. If another process won, its key is read.
pub(crate) fn create_or_adopt(path: &Path, signed_from: i64) -> io::Result<ConfigHistoryKey> {
    let dir = parent_dir(path);
    let epoch = u64::try_from(signed_from)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "negative signing epoch"))?;
    let mut contents = Zeroizing::new([0_u8; KEY_FILE_LEN]);
    getrandom::fill(&mut contents[..KEY_LEN])
        .map_err(|error| io::Error::other(error.to_string()))?;
    contents[KEY_LEN..].copy_from_slice(&epoch.to_be_bytes());

    let mut tmp = tempfile::Builder::new()
        .prefix(TEMP_PREFIX)
        .permissions(Permissions::from_mode(0o600))
        .tempfile_in(dir)?;
    tmp.as_file_mut().write_all(&contents[..])?;
    tmp.as_file().sync_all()?;
    match tmp.persist_noclobber(path) {
        Ok(_) => {
            // Best effort: make the new directory entry durable.
            if let Ok(dir) = File::open(dir) {
                let _ = dir.sync_all();
            }
            let mut bytes = Zeroizing::new([0_u8; KEY_LEN]);
            bytes.copy_from_slice(&contents[..KEY_LEN]);
            Ok(ConfigHistoryKey::new(bytes, signed_from))
        }
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => load(path),
        Err(error) => Err(error.error),
    }
}

/// Removes crash leftovers from key creation: regular files named
/// `.config_history_hmac.key.*`, owned by us, older than [`STALE_TEMP_AGE`].
fn remove_stale_temp_files(key_path: &Path) {
    let Ok(entries) = fs::read_dir(parent_dir(key_path)) else {
        return;
    };
    let euid = nix::unistd::geteuid().as_raw();
    let now = SystemTime::now();
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with(TEMP_PREFIX) {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let stale = meta
            .modified()
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age >= STALE_TEMP_AGE);
        if meta.is_file() && meta.uid() == euid && stale {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn parent_dir(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn hex_decode_mac(hex: &str) -> Option<[u8; MAC_LEN]> {
    let raw = hex.as_bytes();
    if raw.len() != MAC_LEN * 2 {
        return None;
    }
    let mut out = [0_u8; MAC_LEN];
    for (byte, pair) in out.iter_mut().zip(raw.chunks_exact(2)) {
        let hi = (pair[0] as char).to_digit(16)?;
        let lo = (pair[1] as char).to_digit(16)?;
        *byte = (hi * 16 + lo) as u8;
    }
    Some(out)
}
