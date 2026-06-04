#![cfg(test)]

use super::super::*;
use super::fixtures::*;

fn sample_hook(id: &str, stage: HookStage) -> LaunchHook {
    LaunchHook {
        id: id.to_string(),
        name: format!("Hook {id}"),
        path: format!("/usr/local/bin/{id}.sh"),
        stage,
        enabled: true,
    }
}

#[test]
fn launch_hooks_two_each_toml_roundtrip() {
    let mut profile = sample_profile();
    profile.pre_launch_hooks = vec![
        sample_hook("pre-a", HookStage::PreLaunch),
        sample_hook("pre-b", HookStage::PreLaunch),
    ];
    profile.post_exit_hooks = vec![
        sample_hook("post-a", HookStage::PostExit),
        sample_hook("post-b", HookStage::PostExit),
    ];

    let serialized = toml::to_string_pretty(&profile).expect("serialize");

    assert!(
        serialized.contains("[[pre_launch_hooks]]"),
        "expected [[pre_launch_hooks]] table header: {serialized}"
    );
    assert!(
        serialized.contains("[[post_exit_hooks]]"),
        "expected [[post_exit_hooks]] table header: {serialized}"
    );
    assert!(
        serialized.contains(r#"stage = "pre-launch""#),
        "expected kebab-case pre-launch stage value: {serialized}"
    );
    assert!(
        serialized.contains(r#"stage = "post-exit""#),
        "expected kebab-case post-exit stage value: {serialized}"
    );

    let parsed: GameProfile = toml::from_str(&serialized).expect("deserialize");
    assert_eq!(parsed, profile);
}

#[test]
fn empty_hook_vecs_omitted_from_toml() {
    let profile = sample_profile();
    let serialized = toml::to_string_pretty(&profile).expect("serialize");

    assert!(
        !serialized.contains("pre_launch_hooks"),
        "expected empty pre_launch_hooks to be omitted: {serialized}"
    );
    assert!(
        !serialized.contains("post_exit_hooks"),
        "expected empty post_exit_hooks to be omitted: {serialized}"
    );

    let parsed: GameProfile = toml::from_str(&serialized).expect("deserialize");
    assert!(parsed.pre_launch_hooks.is_empty());
    assert!(parsed.post_exit_hooks.is_empty());
}

#[test]
fn legacy_profile_without_hook_keys_defaults_to_empty() {
    let toml = r#"
[game]
executable_path = "/games/test.exe"

[launch]
method = "proton-run"
"#;
    let parsed: GameProfile = toml::from_str(toml).expect("deserialize");
    assert!(parsed.pre_launch_hooks.is_empty());
    assert!(parsed.post_exit_hooks.is_empty());
}

#[test]
fn unknown_stage_value_is_rejected() {
    let toml = r#"
[game]
executable_path = "/games/test.exe"

[launch]
method = "proton-run"

[[pre_launch_hooks]]
id = "hook-1"
name = "Hook 1"
path = "/usr/local/bin/hook.sh"
stage = "mid-flight"
enabled = true
"#;
    let result = toml::from_str::<GameProfile>(toml);
    assert!(
        result.is_err(),
        "expected deserialization to fail for unknown stage variant"
    );
}

#[test]
fn stage_defaults_to_pre_launch_when_omitted() {
    let toml = r#"
[game]
executable_path = "/games/test.exe"

[launch]
method = "proton-run"

[[pre_launch_hooks]]
id = "hook-1"
name = "Hook 1"
path = "/usr/local/bin/hook.sh"
enabled = true
"#;
    let parsed: GameProfile = toml::from_str(toml).expect("deserialize");
    assert_eq!(parsed.pre_launch_hooks.len(), 1);
    assert_eq!(parsed.pre_launch_hooks[0].stage, HookStage::PreLaunch);
    assert_eq!(HookStage::default(), HookStage::PreLaunch);
}

#[test]
fn malformed_hook_missing_identity_dropped() {
    // A hook entry with no `id` deserializes (serde tolerates the missing field via
    // `#[serde(default)]`), but identity-less hooks are unusable — `normalize_hooks`
    // drops them so inconsistent state never reaches downstream use.
    let toml = r#"
[game]
executable_path = "/games/test.exe"

[launch]
method = "proton-run"

[[pre_launch_hooks]]
path = "/usr/local/bin/hook.sh"
"#;
    let mut parsed: GameProfile = toml::from_str(toml).expect("deserialize");
    assert_eq!(
        parsed.pre_launch_hooks.len(),
        1,
        "raw serde still yields the empty-id entry before normalization"
    );

    parsed.normalize_hooks();

    assert!(
        parsed.pre_launch_hooks.is_empty(),
        "identity-less hook must be dropped during normalization"
    );
    assert!(parsed.post_exit_hooks.is_empty());
}

#[test]
fn normalize_hooks_forces_stage_to_container() {
    // A `[[pre_launch_hooks]]` entry authored with the wrong `stage` value must be
    // reconciled to its container's stage — the vec is authoritative.
    let toml = r#"
[game]
executable_path = "/games/test.exe"

[launch]
method = "proton-run"

[[pre_launch_hooks]]
id = "hook-1"
name = "Hook 1"
path = "/usr/local/bin/hook.sh"
stage = "post-exit"
enabled = true

[[post_exit_hooks]]
id = "hook-2"
name = "Hook 2"
path = "/usr/local/bin/hook2.sh"
stage = "pre-launch"
enabled = true
"#;
    let mut parsed: GameProfile = toml::from_str(toml).expect("deserialize");
    assert_eq!(
        parsed.pre_launch_hooks[0].stage,
        HookStage::PostExit,
        "raw serde preserves the mismatched stage before normalization"
    );

    parsed.normalize_hooks();

    assert_eq!(parsed.pre_launch_hooks.len(), 1);
    assert_eq!(parsed.pre_launch_hooks[0].stage, HookStage::PreLaunch);
    assert_eq!(parsed.post_exit_hooks.len(), 1);
    assert_eq!(parsed.post_exit_hooks[0].stage, HookStage::PostExit);
}
