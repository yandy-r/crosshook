use super::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn copies_empty_dir() {
    let t = tempdir().unwrap();
    let src = t.path().join("src");
    let dst = t.path().join("dst");
    fs::create_dir_all(&src).unwrap();

    copy_dir_recursive(&src, &dst).unwrap();

    assert!(dst.exists(), "dst directory should exist");
    let mut entries = fs::read_dir(&dst).unwrap();
    assert!(entries.next().is_none(), "dst should be empty");
}

#[test]
fn copies_nested_files() {
    let t = tempdir().unwrap();
    let src = t.path().join("src");
    let dst = t.path().join("dst");

    fs::create_dir_all(src.join("a/b")).unwrap();
    fs::write(src.join("a/b/c.txt"), "héllo wörld — unicode content").unwrap();

    copy_dir_recursive(&src, &dst).unwrap();

    let content = fs::read_to_string(dst.join("a/b/c.txt")).unwrap();
    assert_eq!(content, "héllo wörld — unicode content");
}

#[cfg(unix)]
#[test]
fn preserves_symlinks() {
    use std::os::unix::fs::symlink;

    let t = tempdir().unwrap();
    let src = t.path().join("src");
    let dst = t.path().join("dst");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("target.txt"), b"data").unwrap();
    symlink("target.txt", src.join("link.txt")).unwrap();

    copy_dir_recursive(&src, &dst).unwrap();

    assert!(
        dst.join("link.txt").is_symlink(),
        "dst entry should remain a symlink"
    );
    let link_target = fs::read_link(dst.join("link.txt")).unwrap();
    assert_eq!(
        link_target,
        std::path::PathBuf::from("target.txt"),
        "symlink target must not be dereferenced"
    );
}

#[test]
fn handles_unicode_names() {
    let t = tempdir().unwrap();
    let src = t.path().join("src");
    let dst = t.path().join("dst");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("résumé.txt"), b"cv data").unwrap();

    copy_dir_recursive(&src, &dst).unwrap();

    assert!(
        dst.join("résumé.txt").exists(),
        "unicode filename must be copied verbatim"
    );
    assert_eq!(fs::read(dst.join("résumé.txt")).unwrap(), b"cv data");
}

#[test]
fn dir_is_empty_true_for_empty_dir() {
    let t = tempdir().unwrap();
    let dir = t.path().join("empty");
    fs::create_dir_all(&dir).unwrap();

    assert!(dir_is_empty(&dir).unwrap());
}

#[test]
fn dir_is_empty_false_when_populated() {
    let t = tempdir().unwrap();
    let dir = t.path().join("nonempty");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("file.txt"), b"x").unwrap();

    assert!(!dir_is_empty(&dir).unwrap());
}

#[test]
fn dir_is_empty_propagates_notfound() {
    let t = tempdir().unwrap();
    let nonexistent = t.path().join("does_not_exist");

    let result = dir_is_empty(&nonexistent);
    assert!(result.is_err(), "expected an error for nonexistent path");
    assert_eq!(
        result.unwrap_err().kind(),
        io::ErrorKind::NotFound,
        "expected NotFound error kind"
    );
}

fn temp_files_in(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains(".tmp-"))
        .collect()
}

#[test]
fn write_atomic_creates_new_file_0644() {
    let t = tempdir().unwrap();
    let path = t.path().join("settings.toml");

    write_atomic(&path, b"hello").unwrap();

    assert_eq!(fs::read(&path).unwrap(), b"hello");
    assert!(temp_files_in(t.path()).is_empty(), "temp file leaked");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644);
    }
}

#[test]
fn write_atomic_preserves_existing_permissions() {
    let t = tempdir().unwrap();
    let path = t.path().join("data.toml");
    fs::write(&path, b"old").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    }

    write_atomic(&path, b"new").unwrap();

    assert_eq!(fs::read(&path).unwrap(), b"new");
    assert!(temp_files_in(t.path()).is_empty(), "temp file leaked");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}

#[cfg(unix)]
#[test]
fn write_atomic_follows_symlink_to_target() {
    use std::os::unix::fs::symlink;

    let t = tempdir().unwrap();
    let real = t.path().join("real.toml");
    fs::write(&real, b"old").unwrap();
    let link = t.path().join("link.toml");
    symlink("real.toml", &link).unwrap();

    write_atomic(&link, b"via-link").unwrap();

    assert!(link.is_symlink(), "symlink must be preserved, not replaced");
    assert_eq!(fs::read_link(&link).unwrap(), PathBuf::from("real.toml"));
    assert_eq!(fs::read(&real).unwrap(), b"via-link");
    assert!(temp_files_in(t.path()).is_empty(), "temp file leaked");
}

#[cfg(unix)]
#[test]
fn write_atomic_rejects_dangling_symlink_without_replacement() {
    use std::os::unix::fs::symlink;

    let t = tempdir().unwrap();
    let link = t.path().join("dangling.toml");
    symlink("nowhere.toml", &link).unwrap();

    let err = write_atomic(&link, b"x").unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::NotFound);
    assert!(link.is_symlink(), "dangling symlink must stay a symlink");
    assert_eq!(fs::read_link(&link).unwrap(), PathBuf::from("nowhere.toml"));
    assert!(!t.path().join("nowhere.toml").exists());
    assert!(
        temp_files_in(t.path()).is_empty(),
        "no temp file should remain after failure"
    );
    assert!(
        err.to_string().contains("symlink"),
        "error should mention the symlink: {err}"
    );
}

#[test]
fn write_atomic_failure_before_rename_keeps_original_and_cleans_temp() {
    let t = tempdir().unwrap();
    let path = t.path().join("profile.toml");
    fs::write(&path, b"original").unwrap();

    let err = write_atomic_inner(&path, b"clobbered", || {
        assert_eq!(temp_files_in(t.path()).len(), 1, "temp should be staged");
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "injected"))
    })
    .unwrap_err();

    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(
        fs::read(&path).unwrap(),
        b"original",
        "original content must survive a pre-rename failure"
    );
    assert!(
        temp_files_in(t.path()).is_empty(),
        "temp file must be removed after a pre-rename failure"
    );
}

#[test]
fn write_atomic_handles_max_length_multibyte_name() {
    let t = tempdir().unwrap();
    // 85 * 3 bytes = 255 bytes: valid name, but too long with temp affixes.
    let path = t.path().join("€".repeat(85));

    write_atomic(&path, b"long").unwrap();

    assert_eq!(fs::read(&path).unwrap(), b"long");
    assert!(temp_files_in(t.path()).is_empty());
    assert_eq!(truncate_on_char_boundary("aé", 2), "a");
}

#[test]
fn write_atomic_replaces_content_and_double_write_is_byte_identical() {
    let t = tempdir().unwrap();
    let path = t.path().join("p.toml");

    write_atomic(&path, b"v1").unwrap();
    let first = fs::read(&path).unwrap();
    write_atomic(&path, b"v1").unwrap();
    let second = fs::read(&path).unwrap();
    assert_eq!(first, second);
    write_atomic(&path, b"v2").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"v2");
    assert!(temp_files_in(t.path()).is_empty());
}
