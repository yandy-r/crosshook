use std::path::Path;

use super::super::{
    extract_tar_read_sync, first_normal_path_component, peek_tar_read_top_level_sync,
    validate_unpack_result, InstallError,
};

#[test]
fn first_normal_component_returns_plain_dir_name() {
    let path = Path::new("GE-Proton9-21/proton");
    assert_eq!(
        first_normal_path_component(path),
        Some("GE-Proton9-21".to_string())
    );
}

#[test]
fn first_normal_component_skips_leading_current_dir_marker() {
    let path = Path::new("./proton-EM-10.0-37-HDR/proton");
    assert_eq!(
        first_normal_path_component(path),
        Some("proton-EM-10.0-37-HDR".to_string())
    );
}

#[test]
fn first_normal_component_rejects_absolute_paths() {
    let path = Path::new("/etc/passwd");
    assert_eq!(first_normal_path_component(path), None);
}

#[test]
fn first_normal_component_rejects_parent_traversal() {
    let path = Path::new("../escape/me");
    assert_eq!(first_normal_path_component(path), None);
}

#[test]
fn peek_tar_with_dot_slash_prefix_returns_real_top_level() {
    let mut buf = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut buf);
        let mut header = tar::Header::new_gnu();
        header.set_path("./proton-EM-10.0-37-HDR/proton").unwrap();
        header.set_size(0);
        header.set_cksum();
        builder.append(&header, std::io::empty()).unwrap();
        builder.finish().unwrap();
    }

    let top = peek_tar_read_top_level_sync(buf.as_slice()).expect("peek must succeed");
    assert_eq!(top, "proton-EM-10.0-37-HDR");
}

#[test]
fn peek_tar_rejects_root_level_regular_file() {
    let mut buf = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut buf);
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_path("proton").expect("file path");
        header.set_size(0);
        header.set_cksum();
        builder.append(&header, std::io::empty()).unwrap();
        builder.finish().unwrap();
    }

    let result = peek_tar_read_top_level_sync(buf.as_slice());
    assert!(
        matches!(result, Err(InstallError::InvalidPath(_))),
        "expected InvalidPath for root-level file, got {result:?}"
    );
}

#[test]
fn extract_tar_rejects_divergent_top_level_directory() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut buf = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut buf);

        for path in ["GE-Proton9-21/proton", "OtherTool/proton"] {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Regular);
            header.set_path(path).expect("file path");
            header.set_size(0);
            header.set_mode(0o755);
            header.set_cksum();
            builder.append(&header, std::io::empty()).unwrap();
        }

        builder.finish().unwrap();
    }

    let result = extract_tar_read_sync(buf.as_slice(), temp.path());
    assert!(
        matches!(result, Err(InstallError::InvalidPath(_))),
        "expected InvalidPath for divergent top-level directories, got {result:?}"
    );
}

#[test]
fn extract_tar_rejects_link_entries_that_escape_install_root() {
    for (entry_type, path, target) in [
        (tar::EntryType::Symlink, "GE-Proton9-21/escape", "/etc"),
        (
            tar::EntryType::Symlink,
            "GE-Proton9-21/escape",
            "../../../etc",
        ),
        (
            tar::EntryType::Link,
            "GE-Proton9-21/hardlink",
            "../outside/proton",
        ),
    ] {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut buf = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut buf);
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(entry_type);
            header.set_path(path).expect("link path");
            header.set_link_name(target).expect("link target");
            header.set_size(0);
            header.set_mode(0o777);
            header.set_cksum();
            builder
                .append(&header, std::io::empty())
                .expect("append link entry");
            builder.finish().expect("finish tar");
        }

        let result = extract_tar_read_sync(buf.as_slice(), temp.path());
        assert!(
            matches!(result, Err(InstallError::InvalidPath(_))),
            "expected InvalidPath for tar link entry {path}, got {result:?}"
        );
    }
}

#[test]
fn extract_tar_allows_safe_symlink_entries_within_archive_root() {
    let temp = tempfile::tempdir().expect("temp dir");
    let mut buf = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut buf);

        let mut dir_header = tar::Header::new_gnu();
        dir_header.set_entry_type(tar::EntryType::Directory);
        dir_header
            .set_path("GE-Proton9-21/usr/bin")
            .expect("dir path");
        dir_header.set_size(0);
        dir_header.set_mode(0o755);
        dir_header.set_cksum();
        builder
            .append(&dir_header, std::io::empty())
            .expect("append directory");

        let mut symlink_header = tar::Header::new_gnu();
        symlink_header.set_entry_type(tar::EntryType::Symlink);
        symlink_header
            .set_path("GE-Proton9-21/bin")
            .expect("symlink path");
        symlink_header
            .set_link_name("usr/bin")
            .expect("symlink target");
        symlink_header.set_size(0);
        symlink_header.set_mode(0o777);
        symlink_header.set_cksum();
        builder
            .append(&symlink_header, std::io::empty())
            .expect("append symlink");

        let payload = b"proton-binary";
        let mut file_header = tar::Header::new_gnu();
        file_header.set_entry_type(tar::EntryType::Regular);
        file_header
            .set_path("GE-Proton9-21/bin/proton")
            .expect("file path");
        file_header.set_size(payload.len() as u64);
        file_header.set_mode(0o755);
        file_header.set_cksum();
        builder
            .append(&file_header, payload.as_slice())
            .expect("append payload");

        builder.finish().expect("finish tar");
    }

    let extracted = extract_tar_read_sync(buf.as_slice(), temp.path()).expect("extract tar");
    assert_eq!(extracted, "GE-Proton9-21");
    assert_eq!(
        std::fs::read(temp.path().join("GE-Proton9-21/usr/bin/proton"))
            .expect("read extracted payload"),
        b"proton-binary"
    );
}

#[test]
fn validate_unpack_result_rejects_skipped_entries() {
    let result = validate_unpack_result(Path::new("GE-Proton9-21/../../escape"), false);
    assert!(
        matches!(result, Err(InstallError::InvalidPath(_))),
        "expected InvalidPath for skipped traversal entry, got {result:?}"
    );
}
