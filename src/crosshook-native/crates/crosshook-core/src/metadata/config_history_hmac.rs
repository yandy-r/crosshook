//! HMAC-SHA256 tamper evidence for `config_revisions` (ADR-0005).
//!
//! The 32-byte key is a filesystem secret stored next to `metadata.db` as
//! [`KEY_FILE_NAME`] (`0600`). It never enters SQLite, logs, or IPC. Stored
//! signatures are self-describing: `v1:<key_id>:<hex_mac>`, where `key_id` is a
//! short SHA-256 fingerprint of the key (not secret-revealing).

use std::fmt::{self, Debug, Formatter, Write as _};
use std::fs::{File, OpenOptions, Permissions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};

/// Key file name, created in the same directory as the metadata database.
pub const KEY_FILE_NAME: &str = "config_history_hmac.key";

const KEY_LEN: usize = 32;
const MAC_LEN: usize = 32;
/// Bytes of SHA-256(key) used as the public key fingerprint (16 hex chars).
const KEY_ID_LEN: usize = 8;
const SIGNATURE_VERSION: &str = "v1";
const MESSAGE_TAG: &[u8] = b"crosshook.config_revision.v1";

type HmacSha256 = Hmac<Sha256>;

/// Machine-local config-history signing key. `Debug` prints only the key id.
pub(crate) struct ConfigHistoryKey {
    bytes: [u8; KEY_LEN],
    id: String,
}

/// Result of checking a stored signature against the current key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SignatureCheck {
    Valid,
    Mismatch,
    UnknownVersion,
    ForeignKey,
}

impl Debug for ConfigHistoryKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigHistoryKey")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl ConfigHistoryKey {
    fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        let digest = Sha256::digest(bytes);
        Self {
            id: hex_encode(&digest[..KEY_ID_LEN]),
            bytes,
        }
    }

    /// Public, non-secret fingerprint embedded in signatures.
    #[cfg(test)]
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    fn mac(&self, profile_id: &str, content_hash: &str, snapshot_toml: &str) -> HmacSha256 {
        let mut mac =
            HmacSha256::new_from_slice(&self.bytes).expect("HMAC accepts keys of any length");
        for field in [
            MESSAGE_TAG,
            profile_id.as_bytes(),
            content_hash.as_bytes(),
            snapshot_toml.as_bytes(),
        ] {
            mac.update(&(field.len() as u64).to_be_bytes());
            mac.update(field);
        }
        mac
    }

    /// Signs a revision, returning the stored `v1:<key_id>:<hex_mac>` form.
    pub(crate) fn sign(&self, profile_id: &str, content_hash: &str, snapshot_toml: &str) -> String {
        let tag = self
            .mac(profile_id, content_hash, snapshot_toml)
            .finalize()
            .into_bytes();
        format!("{SIGNATURE_VERSION}:{}:{}", self.id, hex_encode(&tag))
    }

    /// Checks a stored signature in constant time (`verify_slice`).
    pub(crate) fn check(
        &self,
        signature: &str,
        profile_id: &str,
        content_hash: &str,
        snapshot_toml: &str,
    ) -> SignatureCheck {
        let mut parts = signature.splitn(3, ':');
        let (Some(version), Some(key_id), Some(hex_mac)) =
            (parts.next(), parts.next(), parts.next())
        else {
            return SignatureCheck::UnknownVersion;
        };
        if version != SIGNATURE_VERSION {
            return SignatureCheck::UnknownVersion;
        }
        if key_id != self.id {
            return SignatureCheck::ForeignKey;
        }
        // Same key, so a malformed MAC is tampering, not an unknown format.
        let Some(tag) = hex_decode_mac(hex_mac) else {
            return SignatureCheck::Mismatch;
        };
        match self
            .mac(profile_id, content_hash, snapshot_toml)
            .verify_slice(&tag)
        {
            Ok(()) => SignatureCheck::Valid,
            Err(_) => SignatureCheck::Mismatch,
        }
    }
}

/// Path of the key file for the database at `db_path`.
pub(crate) fn key_path(db_path: &Path) -> PathBuf {
    db_path.with_file_name(KEY_FILE_NAME)
}

/// Loads the key for a file-backed store. Writable stores create it when absent;
/// read-only stores only load. Failures are logged (never key material) and
/// leave the store on the unsigned / hash-only path.
pub(super) fn open_for_store(db_path: &Path, writable: bool) -> Option<Arc<ConfigHistoryKey>> {
    let path = key_path(db_path);
    let result = if writable {
        load_or_create(&path)
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
pub(crate) fn load_or_create(path: &Path) -> io::Result<ConfigHistoryKey> {
    match load(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => create_or_adopt(path),
        result => result,
    }
}

/// Reads exactly [`KEY_LEN`] bytes from a regular, non-symlink key file.
pub(crate) fn load(path: &Path) -> io::Result<ConfigHistoryKey> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "config history key is not a regular file",
        ));
    }
    let mut buf = [0_u8; KEY_LEN + 1];
    let mut len = 0;
    while len < buf.len() {
        match file.read(&mut buf[len..])? {
            0 => break,
            n => len += n,
        }
    }
    if len != KEY_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "config history key has an invalid length",
        ));
    }
    let mut bytes = [0_u8; KEY_LEN];
    bytes.copy_from_slice(&buf[..KEY_LEN]);
    Ok(ConfigHistoryKey::from_bytes(bytes))
}

/// Generates a key and publishes it without ever replacing an existing file:
/// the key is written and fsynced to a private temp file, then linked into
/// place with no-replace semantics. If another process won, its key is read.
pub(crate) fn create_or_adopt(path: &Path) -> io::Result<ConfigHistoryKey> {
    let dir = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let mut bytes = [0_u8; KEY_LEN];
    getrandom::fill(&mut bytes).map_err(|error| io::Error::other(error.to_string()))?;

    let mut tmp = tempfile::Builder::new()
        .prefix(".config_history_hmac.key.")
        .permissions(Permissions::from_mode(0o600))
        .tempfile_in(dir)?;
    tmp.as_file_mut().write_all(&bytes)?;
    tmp.as_file().sync_all()?;
    match tmp.persist_noclobber(path) {
        Ok(_) => {
            // Best effort: make the new directory entry durable.
            if let Ok(dir) = File::open(dir) {
                let _ = dir.sync_all();
            }
            Ok(ConfigHistoryKey::from_bytes(bytes))
        }
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => load(path),
        Err(error) => Err(error.error),
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
