//! Settings screen flows (F1-F9) driven through `SettingsViewModel` with
//! `testing::Script`.

use super::{
    LogFilter, SettingsEffect, SettingsField, SettingsFields, SettingsIntent, SettingsSaveRequest,
    SettingsViewModel, RECENT_FILES_LIMIT_MESSAGE,
};
use crate::error::{ServiceError, ServiceErrorKind};
use crate::form::{FormError, FormModel, SubmitId, SubmitState};
use crate::testing::Script;
use crate::ViewModel;

fn loaded() -> SettingsFields {
    SettingsFields {
        profiles_directory: String::new(),
        recent_files_limit: "10".to_owned(),
        log_filter: LogFilter::Info,
    }
}

fn vm() -> SettingsViewModel {
    SettingsViewModel::new(loaded())
}

/// Edits the directory, submits, and returns the id of the emitted save.
fn submit_directory(vm: &mut SettingsViewModel, dir: &str) -> SubmitId {
    vm.reduce(SettingsIntent::EditProfilesDirectory(dir.to_owned()));
    match vm.reduce(SettingsIntent::Submit).as_slice() {
        [SettingsEffect::Save { id, .. }] => *id,
        other => panic!("expected one Save, got {other:?}"),
    }
}

fn finished(id: SubmitId, result: Result<(), ServiceError>) -> SettingsIntent {
    SettingsIntent::SaveFinished { id, result }
}

#[test]
fn submit_emits_parsed_trimmed_request() {
    let mut vm = vm();
    vm.reduce(SettingsIntent::EditProfilesDirectory(
        "  ~/games  ".to_owned(),
    ));
    vm.reduce(SettingsIntent::EditRecentFilesLimit(" 25 ".to_owned()));
    vm.reduce(SettingsIntent::EditLogFilter(LogFilter::Debug));

    let effects = vm.reduce(SettingsIntent::Submit);

    let [SettingsEffect::Save { id, request }] = effects.as_slice() else {
        panic!("expected one Save, got {effects:?}");
    };
    assert_eq!(
        request,
        &SettingsSaveRequest {
            profiles_directory: "~/games".to_owned(),
            recent_files_limit: 25,
            log_filter: LogFilter::Debug,
        }
    );
    assert_eq!(vm.form().submit_state(), SubmitState::Submitting(*id));
}

#[test]
fn empty_profiles_directory_means_default_not_an_error() {
    let mut vm = vm();
    vm.reduce(SettingsIntent::EditProfilesDirectory("   ".to_owned()));

    let effects = vm.reduce(SettingsIntent::Submit);

    assert!(matches!(
        effects.as_slice(),
        [SettingsEffect::Save { request, .. }] if request.profiles_directory.is_empty()
    ));
}

#[test]
fn backend_profiles_directory_error_lands_on_field() {
    let mut vm = vm();
    let id = submit_directory(&mut vm, "/nope");

    let vm = Script::new(vm)
        .step(
            finished(
                id,
                Err(ServiceError::validation(
                    "profiles_directory",
                    "Directory is not writable.",
                )),
            ),
            [],
        )
        .into_inner();

    let form = vm.form();
    assert_eq!(form.submit_state(), SubmitState::Failed);
    assert_eq!(
        form.error(SettingsField::ProfilesDirectory),
        Some("Directory is not writable.")
    );
    assert!(form.is_touched(SettingsField::ProfilesDirectory));
    assert_eq!(form.form_error(), None);
}

#[test]
fn entity_and_unknown_validation_ids_show_banner_verbatim() {
    for field in ["settings", "profile_directory"] {
        let mut vm = vm();
        let id = submit_directory(&mut vm, "/x");

        vm.reduce(finished(
            id,
            Err(ServiceError::validation(field, "Settings are corrupt.")),
        ));

        let form = vm.form();
        assert_eq!(
            form.form_error(),
            Some(&FormError {
                kind: ServiceErrorKind::Validation { field },
                message: "Settings are corrupt.".to_owned(),
            }),
            "{field:?}"
        );
        assert!(
            SettingsFields::FIELDS
                .iter()
                .all(|&f| form.error(f).is_none()),
            "{field:?}"
        );
    }
}

#[test]
fn service_failure_kinds_show_banner_and_clear_field_errors() {
    for kind in [
        ServiceErrorKind::NotFound,
        ServiceErrorKind::Conflict,
        ServiceErrorKind::Unavailable,
        ServiceErrorKind::Io,
        ServiceErrorKind::Internal,
    ] {
        let mut vm = vm();
        let first = submit_directory(&mut vm, "/a");
        vm.reduce(finished(
            first,
            Err(ServiceError::validation("profiles_directory", "bad")),
        ));
        // Resubmit without editing so the field error is still shown going in.
        assert_eq!(
            vm.form().error(SettingsField::ProfilesDirectory),
            Some("bad")
        );
        let [SettingsEffect::Save { id, .. }] = vm.reduce(SettingsIntent::Submit)[..] else {
            panic!("{kind:?}: expected one Save");
        };

        vm.reduce(finished(
            id,
            Err(ServiceError::new(kind, "Settings store unavailable.")),
        ));

        let form = vm.form();
        assert_eq!(form.submit_state(), SubmitState::Failed, "{kind:?}");
        assert_eq!(form.form_error().map(|e| e.kind), Some(kind));
        assert_eq!(
            form.form_error().map(|e| e.message.as_str()),
            Some("Settings store unavailable.")
        );
        assert_eq!(
            form.error(SettingsField::ProfilesDirectory),
            None,
            "{kind:?}"
        );
    }
}

#[test]
fn cancelled_save_returns_to_idle_without_banner() {
    let mut vm = vm();
    let id = submit_directory(&mut vm, "/a");

    vm.reduce(finished(
        id,
        Err(ServiceError::new(ServiceErrorKind::Cancelled, "cancelled")),
    ));

    assert_eq!(vm.form().submit_state(), SubmitState::Idle);
    assert_eq!(vm.form().form_error(), None);
    assert!(vm.form().can_submit());
}

#[test]
fn double_submit_emits_one_save() {
    let mut vm = vm();
    vm.reduce(SettingsIntent::EditRecentFilesLimit("20".to_owned()));
    let request = SettingsSaveRequest {
        profiles_directory: String::new(),
        recent_files_limit: 20,
        log_filter: LogFilter::Info,
    };
    let Some((first_id, _)) = vm.clone().form.begin_submit() else {
        panic!("form is submittable");
    };

    Script::new(vm)
        .step(
            SettingsIntent::Submit,
            [SettingsEffect::Save {
                id: first_id,
                request,
            }],
        )
        .step(SettingsIntent::Submit, [])
        .step(SettingsIntent::Submit, [])
        .assert_state(|vm| {
            assert_eq!(vm.form().submit_state(), SubmitState::Submitting(first_id));
        })
        .into_inner();
}

#[test]
fn stale_save_result_is_ignored() {
    let mut vm = vm();
    let old = submit_directory(&mut vm, "/a");
    vm.reduce(finished(
        old,
        Err(ServiceError::new(ServiceErrorKind::Io, "disk")),
    ));
    let current = submit_directory(&mut vm, "/b");

    Script::new(vm)
        .step(finished(old, Ok(())), [])
        .step(
            finished(old, Err(ServiceError::new(ServiceErrorKind::Io, "late"))),
            [],
        )
        .assert_state(|vm| {
            let form = vm.form();
            assert_eq!(form.submit_state(), SubmitState::Submitting(current));
            assert_eq!(form.form_error(), None);
            assert_eq!(form.baseline(), &loaded());
        })
        .step(finished(current, Ok(())), [])
        .assert_state(|vm| {
            assert_eq!(vm.form().submit_state(), SubmitState::Succeeded);
            assert_eq!(vm.form().baseline().profiles_directory, "/b");
        })
        .into_inner();
}

#[test]
fn load_during_submit_makes_its_result_stale() {
    let mut vm = vm();
    let id = submit_directory(&mut vm, "/a");
    let reloaded = SettingsFields {
        log_filter: LogFilter::Warn,
        ..loaded()
    };

    vm.reduce(SettingsIntent::Loaded(reloaded.clone()));
    vm.reduce(finished(id, Ok(())));

    assert_eq!(vm.form().submit_state(), SubmitState::Idle);
    assert_eq!(vm.form().baseline(), &reloaded);
    assert!(!vm.form().is_form_dirty());
}

#[test]
fn success_rebaselines_and_edits_during_submit_stay_dirty() {
    let mut vm = vm();
    let id = submit_directory(&mut vm, "/a");
    vm.reduce(SettingsIntent::EditLogFilter(LogFilter::Trace));
    assert_eq!(vm.form().submit_state(), SubmitState::Submitting(id));

    vm.reduce(finished(id, Ok(())));

    let form = vm.form();
    assert_eq!(form.submit_state(), SubmitState::Succeeded);
    assert_eq!(form.baseline().profiles_directory, "/a");
    assert_eq!(form.baseline().log_filter, LogFilter::Info);
    assert!(!form.is_dirty(SettingsField::ProfilesDirectory));
    assert!(form.is_dirty(SettingsField::LogFilter));

    // Selecting the saved value again makes the select pristine.
    vm.reduce(SettingsIntent::EditLogFilter(LogFilter::Info));
    assert!(!vm.form().is_form_dirty());
    assert_eq!(vm.form().submit_state(), SubmitState::Idle);
}

#[test]
fn client_invalid_limit_blocks_save_and_shows_error() {
    for text in ["", "abc", "0", "101", "-5", "2.5"] {
        let mut vm = vm();
        vm.reduce(SettingsIntent::EditProfilesDirectory("/a".to_owned()));
        vm.reduce(SettingsIntent::EditRecentFilesLimit(text.to_owned()));

        let vm = Script::new(vm)
            .step(SettingsIntent::Submit, [])
            .into_inner();

        let form = vm.form();
        assert_eq!(form.submit_state(), SubmitState::Failed, "{text:?}");
        assert_eq!(
            form.error(SettingsField::RecentFilesLimit),
            Some(RECENT_FILES_LIMIT_MESSAGE),
            "{text:?}"
        );
        assert!(
            SettingsFields::FIELDS.iter().all(|&f| form.is_touched(f)),
            "{text:?}"
        );
    }
}

#[test]
fn limit_bounds_are_accepted() {
    for (text, limit) in [("1", 1), ("100", 100)] {
        let mut vm = vm();
        vm.reduce(SettingsIntent::EditRecentFilesLimit(text.to_owned()));

        let effects = vm.reduce(SettingsIntent::Submit);

        assert!(
            matches!(
                effects.as_slice(),
                [SettingsEffect::Save { request, .. }] if request.recent_files_limit == limit
            ),
            "{text:?}: {effects:?}"
        );
    }
}

#[test]
fn editing_invalid_field_clears_only_its_error() {
    let mut vm = vm();
    vm.reduce(SettingsIntent::EditRecentFilesLimit("0".to_owned()));
    assert!(vm.reduce(SettingsIntent::Submit).is_empty());

    vm.reduce(SettingsIntent::EditLogFilter(LogFilter::Warn));

    assert_eq!(vm.form().submit_state(), SubmitState::Idle);
    assert_eq!(
        vm.form().error(SettingsField::RecentFilesLimit),
        Some(RECENT_FILES_LIMIT_MESSAGE),
        "editing another field keeps this error"
    );

    vm.reduce(SettingsIntent::EditRecentFilesLimit("5".to_owned()));

    assert_eq!(vm.form().error(SettingsField::RecentFilesLimit), None);
}

#[test]
fn blur_touches_only_that_field() {
    let vm = Script::new(vm())
        .step(SettingsIntent::Blur(SettingsField::RecentFilesLimit), [])
        .into_inner();

    assert!(vm.form().is_touched(SettingsField::RecentFilesLimit));
    assert!(!vm.form().is_touched(SettingsField::ProfilesDirectory));
}

#[test]
fn every_field_id_round_trips_through_apply_errors() {
    for &field in SettingsFields::FIELDS {
        let mut vm = vm();
        let id = submit_directory(&mut vm, "/a");

        vm.reduce(finished(
            id,
            Err(ServiceError::validation(
                SettingsFields::field_id(field),
                "bad",
            )),
        ));

        assert_eq!(vm.form().error(field), Some("bad"), "{field:?}");
        assert_eq!(vm.form().form_error(), None, "{field:?}");
    }
}

#[test]
fn load_failure_puts_the_profiles_directory_error_on_its_field() {
    let error = ServiceError::validation("profiles_directory", "Directory is not usable.");
    let vm = Script::new(vm())
        .step(SettingsIntent::LoadFailed(error), [])
        .into_inner();

    let form = vm.form();
    assert_eq!(
        form.error(SettingsField::ProfilesDirectory),
        Some("Directory is not usable.")
    );
    assert!(form.is_touched(SettingsField::ProfilesDirectory));
    assert_eq!(form.form_error(), None);
    assert_eq!(form.submit_state(), SubmitState::Idle);
    assert_eq!(form.value(), &loaded());
}

#[test]
fn load_failure_without_a_field_shows_a_banner() {
    let error = ServiceError::new(ServiceErrorKind::Io, "settings file unreadable");
    let vm = Script::new(vm())
        .step(SettingsIntent::LoadFailed(error), [])
        .into_inner();

    assert_eq!(
        vm.form().form_error(),
        Some(&FormError {
            kind: ServiceErrorKind::Io,
            message: "settings file unreadable".to_owned(),
        })
    );
}

#[test]
fn load_failure_during_submit_keeps_the_save_in_flight() {
    let mut vm = vm();
    let id = submit_directory(&mut vm, "/a");
    let error = ServiceError::validation("profiles_directory", "Directory is not usable.");

    let vm = Script::new(vm)
        .step(SettingsIntent::LoadFailed(error), [])
        .assert_state(|vm| {
            let form = vm.form();
            assert_eq!(form.submit_state(), SubmitState::Submitting(id));
            assert_eq!(
                form.error(SettingsField::ProfilesDirectory),
                Some("Directory is not usable.")
            );
        })
        .step(SettingsIntent::Submit, [])
        .step(finished(id, Ok(())), [])
        .into_inner();

    let form = vm.form();
    assert_eq!(form.submit_state(), SubmitState::Succeeded);
    assert_eq!(form.baseline().profiles_directory, "/a");
    assert!(!form.is_form_dirty());
}

#[test]
fn cancelled_load_during_submit_allows_no_second_save() {
    let mut vm = vm();
    let id = submit_directory(&mut vm, "/a");
    let error = ServiceError::new(ServiceErrorKind::Cancelled, "cancelled");

    Script::new(vm)
        .step(SettingsIntent::LoadFailed(error), [])
        .step(SettingsIntent::Submit, [])
        .assert_state(|vm| {
            assert_eq!(vm.form().submit_state(), SubmitState::Submitting(id));
            assert_eq!(vm.form().form_error(), None);
        })
        .into_inner();
}

#[test]
fn log_filter_parse_round_trips_as_str() {
    for &filter in LogFilter::ALL {
        assert_eq!(LogFilter::parse(filter.as_str()), Some(filter));
        assert_eq!(
            LogFilter::parse(&format!("  {} \n", filter.as_str())),
            Some(filter)
        );
    }
    assert_eq!(LogFilter::parse(""), Some(LogFilter::Info));
    assert_eq!(LogFilter::parse("   "), Some(LogFilter::Info));
    for unknown in ["DEBUG", "Info", "crosshook=debug", "warn,crosshook=trace"] {
        assert_eq!(LogFilter::parse(unknown), None, "{unknown}");
    }
}
