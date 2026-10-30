use crate::prefix_deps::runner::sanitize::sanitize_stderr;

#[test]
fn sanitize_stderr_replaces_home_paths() {
    let raw = "error: failed to open /home/user/.wine/drive_c/file.dll: no such file";
    let sanitized = sanitize_stderr(raw);
    assert!(
        !sanitized.contains("/home/"),
        "still contains path: {sanitized}"
    );
    assert!(sanitized.contains("<path>"), "no replacement: {sanitized}");
}

#[test]
fn sanitize_stderr_truncates_long_output() {
    let long = "error: ".to_string() + &"x".repeat(600);
    let sanitized = sanitize_stderr(&long);
    assert!(sanitized.ends_with("...(truncated)"));
    assert!(sanitized.len() <= 515); // 500 chars + "...(truncated)"
}
