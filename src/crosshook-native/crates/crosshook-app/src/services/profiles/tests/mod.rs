//! `ProfileWriter` tests. `helpers` is the shared harness; each B1 slice
//! owns its own file (see `docs/plans/profile-writer/b0-contracts.md`).

mod events;
mod helpers;
mod history;
mod imports;
mod lifecycle;
mod migration;
mod names;
mod sections;
mod suggestions;
mod writer;
