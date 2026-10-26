//! CLI output compatibility for YAN-798 (D10), captured from baseline a652cd0.
//!
//! The 13 cases drive the unmodified CLI through its real binary with isolated
//! child environments and stub executables. Fixtures are frozen: a mismatch is
//! a regression, never a reason to regenerate them. Only the temporary root and
//! D4's two timestamp representations are masked. No launch history is written.
//! Replay, invalid UTF-8, CRLF and unterminated lines are intentionally excluded.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crosshook_core::metadata::MetadataStore;
use serde_json::Value;

const CASES: &[&str] = &[
    "d1", "d2", "d3", "d4", "e1", "e2", "e3", "s1", "s2", "s3", "n1", "n2", "p1",
];

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/launch")
}

fn executable(path: &Path, script: &str) {
    fs::write(path, script).expect("write stub executable");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod stub executable");
}

fn prepare(root: &Path, case: &str) {
    for dir in [
        "home/.config",
        "home/.local/share",
        "run",
        "bin",
        "profiles",
        "prefix",
        "steam/steamapps/compatdata/123/pfx",
        "scripts",
    ] {
        fs::create_dir_all(root.join(dir)).expect("create isolated directory");
    }
    executable(
        &root.join("proton"),
        "#!/bin/sh\nprintf 'proton output\\n'\nexit 0\n",
    );
    executable(&root.join("steam/steam.sh"), "#!/bin/sh\nexit 0\n");
    executable(
        &root.join("game.sh"),
        if case == "n2" {
            "#!/bin/sh\nprintf 'Permission denied\\n'\nexit 3\n"
        } else {
            "#!/bin/sh\nprintf 'native output\\n'\nexit 0\n"
        },
    );
    fs::write(root.join("Game.exe"), b"golden game").expect("write game placeholder");
    fs::write(root.join("Trainer.exe"), b"golden trainer").expect("write trainer placeholder");
    let helper = root.join("scripts/steam-launch-helper.sh");
    if case == "s3" {
        fs::create_dir(&helper).expect("non-regular helper fixture");
    } else {
        let result = if case == "s2" {
            "exit 3\n"
        } else {
            "printf '[steam-helper] Steam is not running\\n' >>\"$log_file\"\nexit 0\n"
        };
        executable(
            &helper,
            &format!(
                "#!/bin/sh\nlog_file=\nwhile [ \"$#\" -gt 0 ]; do\n  if [ \"$1\" = --log-file ]; then\n    log_file=$2\n    shift 2\n  else\n    shift\n  fi\ndone\n{result}"
            ),
        );
    }
    if case != "e1" && case != "e3" {
        let profile = fs::read_to_string(fixtures().join(format!("{case}.toml")))
            .expect("read profile fixture")
            .replace("@ROOT@", root.to_str().expect("UTF-8 temp root"));
        fs::write(root.join(format!("profiles/{case}.toml")), profile).expect("install profile");
    }
}

fn command(root: &Path, case: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_crosshook"));
    command
        .env_clear()
        .current_dir(root)
        .env("HOME", root.join("home"))
        .env("XDG_CONFIG_HOME", root.join("home/.config"))
        .env("XDG_DATA_HOME", root.join("home/.local/share"))
        .env("XDG_RUNTIME_DIR", root.join("run"))
        .env("PATH", format!("{}/bin:/usr/bin:/bin", root.display()))
        .env("USER", "tester")
        .env("LOGNAME", "tester")
        .env("SHELL", "/bin/bash")
        .env("DISPLAY", ":99")
        .env("WAYLAND_DISPLAY", "wayland-test")
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            format!("unix:path={}/bus", root.display()),
        )
        .arg("launch")
        .arg("--profile-dir")
        .arg(root.join("profiles"));
    if case != "e3" {
        command
            .arg("--profile")
            .arg(if case == "e1" { "missing" } else { case });
    }
    if case.starts_with('d') {
        command.arg("--dry-run");
    }
    if case == "d4" {
        command.arg("--json");
    }
    if case.starts_with('s') {
        command.arg("--scripts-dir").arg(root.join("scripts"));
    }
    command
}

fn mask_json(stdout: &str) -> Value {
    let mut value: Value = serde_json::from_str(stdout).expect("D4 preview JSON");
    let timestamp = value["generated_at"]
        .as_str()
        .expect("preview timestamp")
        .to_owned();
    let display = value["display_text"]
        .as_str()
        .expect("preview display text");
    let timestamp_line = format!("generated_at = \"{timestamp}\"");
    assert!(
        display.contains(&timestamp_line),
        "display timestamp must be masked too"
    );
    value["display_text"] =
        Value::String(display.replace(&timestamp_line, "generated_at = \"@TIMESTAMP@\""));
    value["generated_at"] = Value::String("@TIMESTAMP@".to_owned());
    value
}

#[test]
fn launch_output_matches_frozen_baseline() {
    if Path::new("/.flatpak-info").exists() {
        eprintln!("CLI goldens require a non-Flatpak host");
        return;
    }
    for &case in CASES {
        let root = tempfile::tempdir().expect("isolated case root");
        prepare(root.path(), case);
        let output = command(root.path(), case)
            .output()
            .expect("run CLI fixture");
        let stdout = String::from_utf8(output.stdout)
            .expect("UTF-8 fixture stdout")
            .replace(root.path().to_str().unwrap(), "@ROOT@");
        let stderr = String::from_utf8(output.stderr)
            .expect("UTF-8 fixture stderr")
            .replace(root.path().to_str().unwrap(), "@ROOT@");
        let code = format!(
            "{}\n",
            output.status.code().expect("CLI exited without signal")
        );
        let stdout = if case == "d4" {
            format!(
                "{}\n",
                serde_json::to_string_pretty(&mask_json(&stdout)).unwrap()
            )
        } else {
            stdout
        };
        for (extension, actual) in [("stdout", &stdout), ("stderr", &stderr), ("code", &code)] {
            let path = fixtures().join(format!("{case}.{extension}"));
            let expected = fs::read_to_string(&path).expect("read frozen fixture");
            if case == "d4" && extension == "stdout" {
                assert_eq!(
                    serde_json::from_str::<Value>(actual).unwrap(),
                    serde_json::from_str::<Value>(&expected).unwrap(),
                    "{case}.{extension}"
                );
                let json = fs::read_to_string(fixtures().join("d4.json")).unwrap();
                assert_eq!(
                    serde_json::from_str::<Value>(actual).unwrap(),
                    serde_json::from_str::<Value>(&json).unwrap(),
                    "d4.json"
                );
            } else {
                assert_eq!(actual, &expected, "{case}.{extension}");
            }
        }
        if case.starts_with(['s', 'n', 'p']) {
            let path = root.path().join("home/.local/share/crosshook/metadata.db");
            assert!(
                path.is_file(),
                "{case}: CLI must initialize its metadata database"
            );
            let store = MetadataStore::with_path(&path).expect("open fixture metadata");
            assert!(
                store
                    .query_launch_history_for_profile(case, 100)
                    .expect("query launch history")
                    .is_empty(),
                "{case}: CLI must not record launch history (YAN-837)"
            );
        }
    }
}
