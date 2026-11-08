use super::*;

#[test]
fn trainer_guidance_lists_both_loading_modes_and_verification_steps() {
    let tmp = tempfile::tempdir().unwrap();
    let service = service(
        SettingsStore::with_base_path(tmp.path().to_path_buf()),
        MetadataStore::disabled(),
    );

    let guidance = service.trainer_guidance();

    let modes: Vec<&str> = guidance
        .loading_modes
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(modes, ["source_directory", "copy_to_prefix"]);
    assert!(!guidance.trainer_sources.is_empty());
    assert!(!guidance.verification_steps.is_empty());
}
