//! Test harness for view-models: [`Script`] runs intents one step at a time
//! and compares the exact, ordered effects of each step.
//!
//! Available in this crate's tests and, with the `testing` feature, to other
//! crates' tests (for example `crosshook-ui` screen tests).

use std::fmt::Debug;

use crate::ViewModel;

pub mod fixtures;

/// Runs a sequence of intents against a view-model, asserting each step's
/// effects exactly and the state in between.
///
/// ```
/// # use crosshook_view_model::{testing::Script, ViewModel};
/// # #[derive(Default)] struct Counter(u32);
/// # impl ViewModel for Counter {
/// #     type Intent = ();
/// #     type Effect = u32;
/// #     fn reduce(&mut self, (): ()) -> Vec<u32> { self.0 += 1; vec![self.0] }
/// # }
/// let counter = Script::new(Counter::default())
///     .step((), [1])
///     .step((), [2])
///     .assert_state(|c| assert_eq!(c.0, 2))
///     .into_inner();
/// assert_eq!(counter.0, 2);
/// ```
#[derive(Debug)]
pub struct Script<VM> {
    vm: VM,
    steps: usize,
}

impl<VM> Script<VM>
where
    VM: ViewModel,
    VM::Effect: PartialEq + Debug,
{
    /// Starts a script over `vm`.
    pub fn new(vm: VM) -> Self {
        Self { vm, steps: 0 }
    }

    /// Applies `intent` and asserts it returned exactly `expected`, in order.
    ///
    /// # Panics
    /// When the effects differ; the message names the 1-based step number.
    #[track_caller]
    #[must_use]
    pub fn step(
        mut self,
        intent: VM::Intent,
        expected: impl IntoIterator<Item = VM::Effect>,
    ) -> Self {
        self.steps += 1;
        let actual = self.vm.reduce(intent);
        let expected: Vec<VM::Effect> = expected.into_iter().collect();
        assert!(
            actual == expected,
            "step {}: effects differ\n  expected: {expected:?}\n    actual: {actual:?}",
            self.steps
        );
        self
    }

    /// Runs `check` against the current state.
    #[track_caller]
    #[must_use]
    pub fn assert_state(self, check: impl FnOnce(&VM)) -> Self {
        check(&self.vm);
        self
    }

    /// Returns the view-model for further assertions.
    pub fn into_inner(self) -> VM {
        self.vm
    }
}
