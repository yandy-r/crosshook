use crosshook_app::{AppEvent, ProfileChangeReason};
use crosshook_core::profile::GameProfile;

use super::super::events::ProfileChangeKind;
use super::super::state::{AutosaveScope, RevisionHash};
use super::external_changes_from_app_event;
use crate::launch::LaunchMethod;

#[test]
fn converts_every_profile_change_reason_variant() {
    let cases = [
        (ProfileChangeReason::Saved, ProfileChangeKind::Saved),
        (ProfileChangeReason::Deleted, ProfileChangeKind::Deleted),
        (
            ProfileChangeReason::Renamed { from: "Old".into() },
            ProfileChangeKind::Renamed { from: "Old".into() },
        ),
        (
            ProfileChangeReason::Duplicated,
            ProfileChangeKind::Duplicated,
        ),
        (
            ProfileChangeReason::FavoriteToggled,
            ProfileChangeKind::FavoriteToggled,
        ),
        (
            ProfileChangeReason::RolledBack,
            ProfileChangeKind::RolledBack,
        ),
        (
            ProfileChangeReason::PresetApplied,
            ProfileChangeKind::PresetApplied,
        ),
        (ProfileChangeReason::Imported, ProfileChangeKind::Imported),
        (ProfileChangeReason::Migrated, ProfileChangeKind::Migrated),
    ];

    for (reason, expected) in cases {
        assert_eq!(ProfileChangeKind::from(&reason), expected);
    }
}

#[test]
fn converts_resync_and_profiles_changed_events() {
    let resync = AppEvent::Resync { missed: 3 };
    let out = external_changes_from_app_event(&resync, Some(RevisionHash::new("ignored")));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].profile_name, None);
    assert_eq!(out[0].reason, ProfileChangeKind::Resync);
    assert_eq!(out[0].revision, None);

    let broadcast = AppEvent::ProfilesChanged {
        reason: ProfileChangeReason::Saved,
        names: Vec::new(),
    };
    let out = external_changes_from_app_event(&broadcast, Some(RevisionHash::new("r1")));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].profile_name, None);
    assert_eq!(out[0].reason, ProfileChangeKind::Saved);
    assert_eq!(out[0].revision, None);
    assert_eq!(out[0].origin_save_id, None);

    let single = AppEvent::ProfilesChanged {
        reason: ProfileChangeReason::Saved,
        names: vec!["Alpha".into()],
    };
    let out = external_changes_from_app_event(&single, Some(RevisionHash::new("r1")));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].profile_name.as_deref(), Some("Alpha"));
    assert_eq!(out[0].revision, Some(RevisionHash::new("r1")));
    assert_eq!(out[0].origin_save_id, None);

    let named = AppEvent::ProfilesChanged {
        reason: ProfileChangeReason::PresetApplied,
        names: vec!["Alpha".into(), "Beta".into()],
    };
    let out = external_changes_from_app_event(&named, Some(RevisionHash::new("r2")));
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].profile_name.as_deref(), Some("Alpha"));
    assert_eq!(out[0].revision, None);
    assert_eq!(out[1].profile_name.as_deref(), Some("Beta"));
    assert_eq!(out[1].revision, None);
    assert_eq!(out[0].reason, ProfileChangeKind::PresetApplied);

    let unrelated = AppEvent::HealthScanCompleted {
        total: 1,
        healthy: 1,
        stale: 0,
        broken: 0,
    };
    assert!(external_changes_from_app_event(&unrelated, None).is_empty());
}

#[test]
fn builds_scope_from_core_profile_launch_method() {
    let mut profile = GameProfile::default();
    profile.game.executable_path = "/games/foo/game.exe".into();
    let scope = AutosaveScope::from_profile("Foo", "Foo", true, &profile);
    assert_eq!(scope.launch_method, LaunchMethod::ProtonRun);
    assert!(scope.can_autosave());

    profile.steam.enabled = true;
    let scope = AutosaveScope::from_profile("Foo", "Foo", true, &profile);
    assert_eq!(scope.launch_method, LaunchMethod::SteamApplaunch);

    profile.steam.enabled = false;
    profile.game.executable_path = "/usr/bin/native_game".into();
    let scope = AutosaveScope::from_profile("Foo", "Foo", true, &profile);
    assert_eq!(scope.launch_method, LaunchMethod::Native);
}
