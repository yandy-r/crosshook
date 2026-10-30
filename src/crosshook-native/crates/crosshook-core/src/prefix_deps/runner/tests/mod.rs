//! Runner tests. Fake host tools are `sh` scripts in a temp dir.

use std::fs;
use std::path::Path;

mod bounded;
mod check;
mod install;
mod normalize;
mod restore;
mod sanitize;
mod snapshot;

/// Create a fake shell script that acts as winetricks/protontricks.
fn make_fake_binary(dir: &std::path::Path, name: &str, script: &str) -> String {
    let path = dir.join(name);
    fs::write(&path, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
    }
    path.to_string_lossy().into_owned()
}

fn write_registry_fixture(prefix: &Path, body: &str) {
    fs::create_dir_all(prefix).unwrap();
    fs::write(
        prefix.join("system.reg"),
        format!(
            "WINE REGISTRY Version 2\n\n[Software\\\\Microsoft\\\\Windows NT\\\\CurrentVersion] 123\n{body}\n\n[System\\\\CurrentControlSet] 124\n"
        ),
    )
    .unwrap();
}

fn make_install_restore_binary(
    dir: &Path,
    captured_calls: &Path,
    install_exit: i32,
    restore_exit: i32,
    restore_delay_secs: u64,
    update_registry_on_restore: bool,
) -> String {
    let restore_registry = if update_registry_on_restore {
        r#"printf '%s\n' 'WINE REGISTRY Version 2' '' '[Software\\Microsoft\\Windows NT\\CurrentVersion] 123' '"CurrentBuild"="19045"' '"CurrentVersion"="6.3"' '"ProductName"="Microsoft Windows 10"' > "$WINEPREFIX/system.reg""#
    } else {
        ":"
    };
    make_fake_binary(
        dir,
        "winetricks",
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nif [ \"$2\" = \"dotnet48\" ]; then exit {install_exit}; fi\nif [ \"$2\" = \"win10\" ]; then sleep {restore_delay_secs}; {restore_registry}; exit {restore_exit}; fi\nexit 0\n",
            captured_calls.display()
        ),
    )
}
