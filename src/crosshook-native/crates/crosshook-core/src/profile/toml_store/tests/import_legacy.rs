use tempfile::tempdir;

use crate::profile::toml_store::ProfileStore;

#[test]
fn import_legacy_converts_windows_paths_and_saves_toml() {
    let temp_dir = tempdir().unwrap();
    let legacy_path = temp_dir.path().join("elden-ring.profile");
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));

    std::fs::write(
        &legacy_path,
        "GamePath=Z:\\games\\elden-ring\\eldenring.exe\nTrainerPath=Z:/trainers/elden-ring.exe\nDll1Path=\nDll2Path=\nLaunchInject1=True\nLaunchInject2=false\nLaunchMethod=proton_run\nUseSteamMode=True\nSteamAppId=1245620\nSteamCompatDataPath=Z:\\steam\\compatdata\\1245620\nSteamProtonPath=Z:/steam/proton/proton\nSteamLauncherIconPath=Z:\\icons\\elden-ring.png\n",
    )
    .unwrap();

    let imported = store.import_legacy(&legacy_path).unwrap();
    assert_eq!(
        imported.game.executable_path,
        "/games/elden-ring/eldenring.exe"
    );
    assert_eq!(imported.trainer.path, "/trainers/elden-ring.exe");
    assert_eq!(imported.steam.compatdata_path, "/steam/compatdata/1245620");
    assert_eq!(imported.steam.launcher.icon_path, "/icons/elden-ring.png");
    assert_eq!(imported.launch.method, "steam_applaunch");
    assert!(store.base_path.join("elden-ring.toml").exists());
}
