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
