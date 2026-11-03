//! `Form` error mapping and submit FSM (F1-F4, F6, F7, F9 at the form level;
//! the Script flows live in `settings/tests.rs`).

use super::{Form, FormError, FormModel, SubmitId, SubmitState};
use crate::error::{ServiceError, ServiceErrorKind};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Account {
    name: String,
    email: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum AccountField {
    Name,
    Email,
}

impl FormModel for Account {
    type Field = AccountField;
    const FIELDS: &'static [AccountField] = &[AccountField::Name, AccountField::Email];

    fn field_id(field: AccountField) -> &'static str {
        match field {
            AccountField::Name => "name",
            AccountField::Email => "email",
        }
    }

    fn field_changed(&self, other: &Self, field: AccountField) -> bool {
        match field {
            AccountField::Name => self.name != other.name,
            AccountField::Email => self.email != other.email,
        }
    }

    fn validate(&self) -> Vec<(AccountField, &'static str)> {
        let mut errors = Vec::new();
        if self.name.trim().is_empty() {
            errors.push((AccountField::Name, "Name is required."));
        }
        if !self.email.contains('@') {
            errors.push((AccountField::Email, "Enter an email address."));
        }
        errors
    }
}

fn account(name: &str, email: &str) -> Account {
    Account {
        name: name.to_owned(),
        email: email.to_owned(),
    }
}

/// A form with an edited email and a submit in flight.
fn submitting() -> (Form<Account>, SubmitId) {
    let mut form = Form::new(account("ada", "a@x"));
    form.edit(AccountField::Email, |v| v.email = "b@x".to_owned());
    let (id, _) = form.begin_submit().expect("valid and idle");
    (form, id)
}

#[test]
fn known_validation_field_becomes_field_error() {
    let (mut form, id) = submitting();
    let error = ServiceError::validation("email", "Email is taken.");

    form.finish_submit(id, Err(&error));

    assert_eq!(form.submit_state(), SubmitState::Failed);
    assert_eq!(form.error(AccountField::Email), Some("Email is taken."));
    assert_eq!(form.error(AccountField::Name), None);
    assert!(form.is_touched(AccountField::Email));
    assert_eq!(form.form_error(), None);
}

#[test]
fn unknown_validation_field_becomes_form_error_verbatim() {
    for field in ["account", "emial"] {
        let (mut form, id) = submitting();
        let error = ServiceError::validation(field, "Account is locked by ~/x.");

        form.finish_submit(id, Err(&error));

        assert_eq!(form.submit_state(), SubmitState::Failed);
        assert_eq!(
            form.form_error(),
            Some(&FormError {
                kind: ServiceErrorKind::Validation { field },
                message: "Account is locked by ~/x.".to_owned(),
            }),
            "field {field:?}"
        );
        assert!(no_field_errors(&form), "field {field:?}");
    }
}

#[test]
fn non_validation_kinds_become_form_error_and_clear_field_errors() {
    for kind in [
        ServiceErrorKind::NotFound,
        ServiceErrorKind::Conflict,
        ServiceErrorKind::Unavailable,
        ServiceErrorKind::Io,
        ServiceErrorKind::Internal,
    ] {
        let mut form = Form::new(account("ada", "a@x"));
        form.apply_errors(&ServiceError::validation("email", "Email is taken."));
        assert_eq!(form.error(AccountField::Email), Some("Email is taken."));

        form.apply_errors(&ServiceError::new(kind, "Store unavailable."));

        assert_eq!(form.submit_state(), SubmitState::Failed, "{kind:?}");
        assert_eq!(
            form.form_error(),
            Some(&FormError {
                kind,
                message: "Store unavailable.".to_owned(),
            }),
            "{kind:?}"
        );
        assert!(no_field_errors(&form), "{kind:?}");
    }
}

#[test]
fn submit_result_error_goes_through_the_same_mapping() {
    let (mut form, id) = submitting();

    form.finish_submit(
        id,
        Err(&ServiceError::new(ServiceErrorKind::Conflict, "Changed.")),
    );

    assert_eq!(form.submit_state(), SubmitState::Failed);
    assert_eq!(
        form.form_error().map(|e| e.kind),
        Some(ServiceErrorKind::Conflict)
    );
    // The attempt is over, so its late duplicate result is ignored.
    form.finish_submit(id, Ok(()));
    assert_eq!(form.submit_state(), SubmitState::Failed);
    assert_eq!(form.baseline(), &account("ada", "a@x"));
}

#[test]
fn cancelled_returns_to_idle_without_banner() {
    let (mut form, id) = submitting();

    form.finish_submit(
        id,
        Err(&ServiceError::new(ServiceErrorKind::Cancelled, "cancelled")),
    );

    assert_eq!(form.submit_state(), SubmitState::Idle);
    assert_eq!(form.form_error(), None);
    assert!(no_field_errors(&form));
    // The attempt is over: its value is still dirty and can be resubmitted.
    assert!(form.can_submit());
}

#[test]
fn stale_or_unexpected_results_are_ignored() {
    let (mut form, old) = submitting();
    form.finish_submit(old, Err(&ServiceError::new(ServiceErrorKind::Io, "disk")));
    let (_, _) = form.begin_submit().expect("failed form resubmits");
    let current = form.submit_state();

    form.finish_submit(old, Ok(()));
    form.finish_submit(old, Err(&ServiceError::new(ServiceErrorKind::Io, "late")));

    assert_eq!(current, form.submit_state());
    assert!(matches!(current, SubmitState::Submitting(id) if id > old));
    assert_eq!(form.form_error(), None);
    assert_eq!(form.baseline(), &account("ada", "a@x"));

    // A result for an idle form (no attempt in flight) is ignored too.
    let mut idle = Form::new(account("ada", "a@x"));
    idle.finish_submit(
        SubmitId(1),
        Err(&ServiceError::new(ServiceErrorKind::Io, "x")),
    );
    assert_eq!(idle.submit_state(), SubmitState::Idle);
    assert_eq!(idle.form_error(), None);
}

#[test]
fn result_from_before_reset_stays_stale() {
    let (mut form, id) = submitting();
    form.reset(account("cy", "c@x"));
    form.edit(AccountField::Name, |v| v.name = "dee".to_owned());
    let (new_id, _) = form.begin_submit().expect("idle after reset");

    form.finish_submit(id, Ok(()));

    assert_ne!(id, new_id);
    assert_eq!(form.submit_state(), SubmitState::Submitting(new_id));
    assert_eq!(form.baseline(), &account("cy", "c@x"));
}

#[test]
fn success_rebaselines_to_submitted_snapshot() {
    let (mut form, id) = submitting();
    form.edit(AccountField::Name, |v| v.name = "bea".to_owned());

    form.finish_submit(id, Ok(()));

    assert_eq!(form.submit_state(), SubmitState::Succeeded);
    assert_eq!(form.baseline(), &account("ada", "b@x"));
    assert!(!form.is_dirty(AccountField::Email));
    assert!(
        form.is_dirty(AccountField::Name),
        "edit during submit stays dirty"
    );
    assert!(form.can_submit());
}

#[test]
fn edit_back_to_baseline_is_pristine() {
    let mut form = Form::new(account("ada", "a@x"));
    form.edit(AccountField::Name, |v| v.name = "bea".to_owned());
    assert!(form.is_form_dirty());

    form.edit(AccountField::Name, |v| v.name = "ada".to_owned());

    assert!(!form.is_dirty(AccountField::Name));
    assert!(!form.can_submit());
}

#[test]
fn edit_after_failure_clears_only_its_field_error_and_banner() {
    let mut form = Form::new(account("", "bad"));
    assert!(form.begin_submit().is_none());
    assert_eq!(
        form.error(AccountField::Email),
        Some("Enter an email address.")
    );

    form.edit(AccountField::Name, |v| v.name = "ada".to_owned());

    assert_eq!(form.submit_state(), SubmitState::Idle);
    assert_eq!(form.error(AccountField::Name), None);
    assert_eq!(
        form.error(AccountField::Email),
        Some("Enter an email address.")
    );

    form.apply_errors(&ServiceError::new(ServiceErrorKind::Internal, "boom"));
    form.edit(AccountField::Email, |v| v.email = "a@x".to_owned());

    assert_eq!(form.submit_state(), SubmitState::Idle);
    assert_eq!(form.form_error(), None);
}

#[test]
fn edit_during_submit_keeps_submitting_and_errors() {
    let (mut form, id) = submitting();

    form.edit(AccountField::Name, |v| v.name = "bea".to_owned());

    assert_eq!(form.submit_state(), SubmitState::Submitting(id));
    assert!(
        form.begin_submit().is_none(),
        "no second save while in flight"
    );
}

#[test]
fn client_invalid_submit_fails_and_touches_every_field() {
    let mut form = Form::new(account("ada", "a@x"));
    form.edit(AccountField::Name, |v| v.name = "  ".to_owned());

    assert!(form.begin_submit().is_none());

    assert_eq!(form.submit_state(), SubmitState::Failed);
    assert_eq!(form.error(AccountField::Name), Some("Name is required."));
    assert!(Account::FIELDS.iter().all(|&f| form.is_touched(f)));
}

#[test]
fn reset_replaces_baseline_and_clears_state() {
    let mut form = Form::new(account("", "a@x"));
    assert!(form.begin_submit().is_none());
    assert!(!no_field_errors(&form));

    form.reset(account("cy", "c@x"));

    assert_eq!(form.value(), &account("cy", "c@x"));
    assert_eq!(form.baseline(), &account("cy", "c@x"));
    assert_eq!(form.submit_state(), SubmitState::Idle);
    assert!(no_field_errors(&form));
    assert!(Account::FIELDS.iter().all(|&f| !form.is_touched(f)));
}

#[test]
fn every_field_id_round_trips_through_apply_errors() {
    for &field in Account::FIELDS {
        let mut form = Form::new(account("ada", "a@x"));

        form.apply_errors(&ServiceError::validation(Account::field_id(field), "bad"));

        assert_eq!(form.error(field), Some("bad"), "{field:?}");
        assert_eq!(form.form_error(), None, "{field:?}");
    }
}

#[test]
#[should_panic(expected = "field_id must be unique")]
#[cfg(debug_assertions)]
fn duplicate_field_ids_panic_in_debug() {
    #[derive(Clone)]
    struct Dup;
    impl FormModel for Dup {
        type Field = u8;
        const FIELDS: &'static [u8] = &[0, 1];
        fn field_id(_: u8) -> &'static str {
            "same"
        }
        fn field_changed(&self, _: &Self, _: u8) -> bool {
            false
        }
    }

    let _ = Form::new(Dup);
}

/// No field carries an error.
fn no_field_errors(form: &Form<Account>) -> bool {
    Account::FIELDS.iter().all(|&f| form.error(f).is_none())
}
