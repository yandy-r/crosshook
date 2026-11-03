//! Form state with per-field dirty/touched tracking and a submit state machine.
//!
//! A screen describes its fields with [`FormModel`] and owns a [`Form`]. The
//! form never emits effects itself: the screen's view-model calls
//! [`Form::begin_submit`] and turns the returned snapshot into its own save
//! effect, then feeds the result back through [`Form::finish_submit`].
//!
//! Dirty state is derived (value vs baseline, per field), so editing a field
//! back to its original value makes it pristine again. Touched state and
//! errors are stored.
//!
//! Submit state machine:
//!
//! | From                      | Event                                    | To              |
//! | ------------------------- | ---------------------------------------- | --------------- |
//! | `Idle`/`Succeeded`/`Failed` | `begin_submit`, client-valid           | `Submitting(id)` |
//! | `Idle`/`Succeeded`/`Failed` | `begin_submit`, client-invalid         | `Failed`        |
//! | `Submitting(_)`           | `begin_submit`                           | unchanged (`None`) |
//! | `Submitting(id)`          | `finish_submit(id, Ok)`                  | `Succeeded`     |
//! | `Submitting(id)`          | `finish_submit(id, Err(Cancelled))`      | `Idle`          |
//! | `Submitting(id)`          | `finish_submit(id, Err(_))`              | `Failed`        |
//! | `Submitting(x)` or other  | `finish_submit(y != x, _)`               | unchanged       |
//! | `Succeeded`/`Failed`      | `edit`                                   | `Idle`          |

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;

use crate::error::{ServiceError, ServiceErrorKind};

/// Describes a form's value type: its fields, their backend ids, per-field
/// change detection, and client-side checks.
pub trait FormModel: Clone {
    /// One variant per field. `Ord` keeps error and touched maps deterministic.
    type Field: Copy + Eq + Ord + Debug + 'static;

    /// Every field, in display order.
    const FIELDS: &'static [Self::Field];

    /// Backend id of `field`, exactly as `AppErrorKind::Validation { field }`
    /// reports it. This is the only place the string id appears; ids must be
    /// unique within the form.
    fn field_id(field: Self::Field) -> &'static str;

    /// Whether `field` differs between `self` and `other`; drives dirty state.
    fn field_changed(&self, other: &Self, field: Self::Field) -> bool;

    /// Client-side syntactic checks (no I/O). Messages are fixed strings so
    /// user input never ends up in error text. Default: no checks.
    fn validate(&self) -> Vec<(Self::Field, &'static str)> {
        Vec::new()
    }
}

/// Identifies one submit attempt so late results of older attempts are
/// ignored. Ids increase monotonically per [`Form`] and survive
/// [`Form::reset`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubmitId(u64);

/// Where a [`Form`] is in its submit cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubmitState {
    /// No submit in flight and no result to show.
    Idle,
    /// The attempt with this id is in flight; only its result is accepted.
    Submitting(SubmitId),
    /// The last attempt saved; the baseline is the submitted snapshot.
    Succeeded,
    /// The last attempt failed client checks or the service rejected it.
    Failed,
}

/// Form-level error (banner): a service error that names no field of the
/// form. `message` is shown verbatim; it is already sanitized upstream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormError {
    /// Category of the underlying service error.
    pub kind: ServiceErrorKind,
    /// Sanitized, user-safe message.
    pub message: String,
}

/// Editable form state: value, pristine baseline, touched fields, field and
/// form errors, and the submit state machine (see the module docs).
#[derive(Debug, Clone)]
pub struct Form<T: FormModel> {
    baseline: T,
    value: T,
    /// Snapshot handed out by the in-flight `begin_submit`; becomes the
    /// baseline on success.
    submitted: Option<T>,
    touched: BTreeSet<T::Field>,
    field_errors: BTreeMap<T::Field, String>,
    form_error: Option<FormError>,
    submit: SubmitState,
    last_id: u64,
}

impl<T: FormModel> Form<T> {
    /// Starts a pristine, untouched, `Idle` form whose baseline is `value`.
    ///
    /// # Panics
    /// In debug builds, when two fields of `T` share a [`FormModel::field_id`].
    pub fn new(value: T) -> Self {
        debug_assert!(
            field_ids_unique::<T>(),
            "FormModel::field_id must be unique per field"
        );
        Self {
            baseline: value.clone(),
            value,
            submitted: None,
            touched: BTreeSet::new(),
            field_errors: BTreeMap::new(),
            form_error: None,
            submit: SubmitState::Idle,
            last_id: 0,
        }
    }

    /// Current (possibly edited) value.
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Pristine reference the dirty state compares against.
    pub fn baseline(&self) -> &T {
        &self.baseline
    }

    /// Applies `f` to the value as an edit of `field`.
    ///
    /// Outside `Submitting`, this also clears `field`'s error and the form
    /// error, and returns `Succeeded`/`Failed` to `Idle`; other fields'
    /// errors stay. During `Submitting` it only edits, so the value diverges
    /// from the submitted snapshot and stays dirty after a successful save.
    pub fn edit(&mut self, field: T::Field, f: impl FnOnce(&mut T)) {
        f(&mut self.value);
        if matches!(self.submit, SubmitState::Submitting(_)) {
            return;
        }
        self.field_errors.remove(&field);
        self.form_error = None;
        self.submit = SubmitState::Idle;
    }

    /// Marks `field` as touched (for example on blur).
    pub fn touch(&mut self, field: T::Field) {
        self.touched.insert(field);
    }

    /// Starts a submit attempt.
    ///
    /// Returns `None` while a submit is in flight (no second save). Otherwise
    /// clears the previous errors and runs [`FormModel::validate`]: on client
    /// errors it returns `None`, records them, touches every field and goes to
    /// `Failed`. On success it goes to `Submitting(id)` and returns the id
    /// with a snapshot of the value for the screen's save effect.
    pub fn begin_submit(&mut self) -> Option<(SubmitId, T)> {
        if matches!(self.submit, SubmitState::Submitting(_)) {
            return None;
        }
        self.field_errors.clear();
        self.form_error = None;
        let client_errors = self.value.validate();
        if !client_errors.is_empty() {
            for (field, message) in client_errors {
                self.field_errors.insert(field, message.to_owned());
            }
            self.touched.extend(T::FIELDS.iter().copied());
            self.submit = SubmitState::Failed;
            return None;
        }
        self.last_id += 1;
        let id = SubmitId(self.last_id);
        self.submitted = Some(self.value.clone());
        self.submit = SubmitState::Submitting(id);
        Some((id, self.value.clone()))
    }

    /// Applies the result of submit attempt `id`.
    ///
    /// Ignored unless the form is `Submitting(id)` (stale or duplicate
    /// result). `Ok` makes the submitted snapshot the new baseline and goes
    /// to `Succeeded`; `Err` is mapped by [`Form::apply_errors`].
    pub fn finish_submit(&mut self, id: SubmitId, result: Result<(), &ServiceError>) {
        if self.submit != SubmitState::Submitting(id) {
            return;
        }
        match result {
            Ok(()) => {
                if let Some(snapshot) = self.submitted.take() {
                    self.baseline = snapshot;
                }
                self.submit = SubmitState::Succeeded;
            }
            Err(error) => self.apply_errors(error),
        }
    }

    /// Maps one service error onto the form and ends any in-flight attempt
    /// (its late result is then ignored).
    ///
    /// - `Validation { field }` naming a field of the form: that field's
    ///   error (and the field is touched so it shows); `Failed`.
    /// - `Validation` naming no field (e.g. `"settings"`), `NotFound`,
    ///   `Conflict`, `Unavailable`, `Io`, `Internal`: form error with the
    ///   message verbatim; `Failed`.
    /// - `Cancelled`: `Idle`, no error shown, existing errors untouched.
    ///
    /// Every non-cancelled error replaces the previous field and form errors.
    pub fn apply_errors(&mut self, error: &ServiceError) {
        self.submitted = None;
        // Exhaustive on purpose: a new kind must decide where it shows.
        let known_field = match error.kind {
            ServiceErrorKind::Cancelled => {
                self.submit = SubmitState::Idle;
                return;
            }
            ServiceErrorKind::Validation { field } => {
                T::FIELDS.iter().copied().find(|&f| T::field_id(f) == field)
            }
            ServiceErrorKind::NotFound
            | ServiceErrorKind::Conflict
            | ServiceErrorKind::Unavailable
            | ServiceErrorKind::Io
            | ServiceErrorKind::Internal => None,
        };
        self.field_errors.clear();
        self.form_error = None;
        self.submit = SubmitState::Failed;
        match known_field {
            Some(field) => {
                self.field_errors.insert(field, error.message.clone());
                self.touched.insert(field);
            }
            None => {
                self.form_error = Some(FormError {
                    kind: error.kind,
                    message: error.message.clone(),
                });
            }
        }
    }

    /// Replaces baseline and value with `value` (an external reload), clears
    /// touched state and errors, and goes to `Idle`. The id counter is kept,
    /// so a result of an attempt started before the reset stays stale.
    pub fn reset(&mut self, value: T) {
        self.baseline = value.clone();
        self.value = value;
        self.submitted = None;
        self.touched.clear();
        self.field_errors.clear();
        self.form_error = None;
        self.submit = SubmitState::Idle;
    }

    /// Whether `field` differs from the baseline.
    pub fn is_dirty(&self, field: T::Field) -> bool {
        self.baseline.field_changed(&self.value, field)
    }

    /// Whether any field differs from the baseline.
    pub fn is_form_dirty(&self) -> bool {
        T::FIELDS.iter().any(|&field| self.is_dirty(field))
    }

    /// Whether `field` was touched since the last reset.
    pub fn is_touched(&self, field: T::Field) -> bool {
        self.touched.contains(&field)
    }

    /// Error shown on `field`, if any.
    pub fn error(&self, field: T::Field) -> Option<&str> {
        self.field_errors.get(&field).map(String::as_str)
    }

    /// Form-level error (banner), if any.
    pub fn form_error(&self) -> Option<&FormError> {
        self.form_error.as_ref()
    }

    /// Current submit state.
    pub fn submit_state(&self) -> SubmitState {
        self.submit
    }

    /// Whether a submit makes sense: nothing in flight and some field dirty.
    pub fn can_submit(&self) -> bool {
        !matches!(self.submit, SubmitState::Submitting(_)) && self.is_form_dirty()
    }
}

fn field_ids_unique<T: FormModel>() -> bool {
    T::FIELDS.iter().enumerate().all(|(i, &a)| {
        T::FIELDS[i + 1..]
            .iter()
            .all(|&b| T::field_id(a) != T::field_id(b))
    })
}

#[cfg(test)]
mod tests;
