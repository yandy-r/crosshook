use crate::launch::request::models::LaunchCommandArgumentsRequest;
use crate::launch::request::{LaunchRequest, RuntimeLaunchConfig};

#[test]
fn launch_request_umu_preference_serde_default() {
    let toml = "method = \"proton_run\"\n";
    let req: LaunchRequest = toml::from_str(toml).unwrap();
    assert_eq!(req.umu_preference, crate::settings::UmuPreference::Auto);
}

#[test]
fn launch_request_runtime_umu_game_id_roundtrip() {
    use crate::settings::UmuPreference;

    let req = LaunchRequest {
        method: "proton_run".to_string(),
        umu_preference: UmuPreference::Umu,
        runtime: RuntimeLaunchConfig {
            umu_game_id: "custom-7".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };
    let serialized = toml::to_string(&req).unwrap();
    let parsed: LaunchRequest = toml::from_str(&serialized).unwrap();
    assert_eq!(parsed.umu_preference, UmuPreference::Umu);
    assert_eq!(parsed.runtime.umu_game_id, "custom-7");
}

#[test]
fn launch_request_command_arguments_absent_deserializes_to_empty_defaults() {
    let toml = "method = \"proton_run\"\n";
    let req: LaunchRequest = toml::from_str(toml).unwrap();
    assert!(req.command_arguments.is_empty());
    assert!(req.command_arguments.enabled_argument_ids.is_empty());
    assert!(req.command_arguments.custom_args.is_empty());
}

#[test]
fn launch_request_command_arguments_empty_omitted_from_toml_and_roundtrips() {
    let req = LaunchRequest {
        method: "proton_run".to_string(),
        ..Default::default()
    };
    let serialized = toml::to_string(&req).unwrap();
    assert!(
        !serialized.contains("command_arguments"),
        "expected empty command arguments skipped: {serialized}"
    );
    let parsed: LaunchRequest = toml::from_str(&serialized).unwrap();
    assert!(parsed.command_arguments.is_empty());
}

#[test]
fn launch_request_command_arguments_nonempty_toml_roundtrip() {
    let req = LaunchRequest {
        method: "proton_run".to_string(),
        command_arguments: LaunchCommandArgumentsRequest {
            enabled_argument_ids: vec!["force_vulkan".to_string(), "skip_launcher".to_string()],
            custom_args: vec!["-dx11".to_string(), "+set cl_showfps 1".to_string()],
        },
        ..Default::default()
    };
    let serialized = toml::to_string(&req).unwrap();
    assert!(serialized.contains("command_arguments"));
    assert!(serialized.contains("enabled_argument_ids"));
    assert!(serialized.contains("custom_args"));
    let parsed: LaunchRequest = toml::from_str(&serialized).unwrap();
    assert_eq!(parsed.command_arguments, req.command_arguments);
}
