# Dependency policy

Rules for adding, upgrading and auditing Rust dependencies in the native
workspace (`src/crosshook-native/`). The machine-checked part lives in
[`src/crosshook-native/deny.toml`](../../src/crosshook-native/deny.toml) and runs
in [`.github/workflows/deps.yml`](../../.github/workflows/deps.yml).

```bash
cargo install --locked cargo-deny@0.20.2   # once
cargo deny --manifest-path src/crosshook-native/Cargo.toml --locked check
```

## What `cargo deny check` enforces

| Check      | Rule                                                                                                                                                                                                                                                                                                                                           |
| ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| advisories | Any RustSec vulnerability, unsound or yanked crate fails. An unmaintained crate fails when a workspace crate depends on it directly. Every `ignore` entry needs a reason and an exit condition.                                                                                                                                                |
| licenses   | Allowed: MIT, Apache-2.0 (also `WITH LLVM-exception`), BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0, CC0-1.0, BSL-1.0, OFL-1.1 (fonts). Anything else, MPL-2.0 included, needs a per-crate exception with a comment saying why.                                                                                                          |
| bans       | `gtk`, `gtk-sys`, `gdk`, `gdk-sys`, `glib`, `glib-sys`, `gobject-sys`, `gio`, `gio-sys`, `webkit2gtk`, `webkit2gtk-sys` and `rfd` may only be depended on by the Tauri/Classic stack listed in each entry's `wrappers`. A new path from `crosshook-core`, `crosshook-app`, `crosshook-view-model`, `crosshook-cli` or a native UI crate fails. |
| sources    | crates.io only. Git dependencies must be listed in `allow-git`.                                                                                                                                                                                                                                                                                |

The gtk3/glib bans exist because of GitHub #26 (glib 0.18 advisory), which only
closes when `src-tauri` is removed. Until then the bans keep the advisory confined
to the Classic shell. `wrappers` only checks direct parents, so a crate that
reaches glib through an allowed intermediate (for example `pango`) passes
cargo-deny. [`scripts/check-app-deps.sh`](../../scripts/check-app-deps.sh) closes
that gap: it walks the full tree of `crosshook-core`, `crosshook-app`,
`crosshook-view-model` and `crosshook-cli` and rejects the whole gtk3 stack. Add
each new native UI crate to its `CHECKS`. `rfd` is banned outside `tauri-plugin-dialog` because it
links gtk3 directly and Cargo unifies its features, so a native crate cannot use
it without gtk3 while Tauri does. Native file dialogs go through
xdg-desktop-portal (`ashpd`). When `src-tauri` is removed, drop the `wrappers`
entries and turn the bans into plain denies.

zbus must use its `async-io` backend workspace-wide and never enable `tokio`:
Cargo unifies zbus features per binary, and a `tokio`-backed zbus panics in
every zbus user that runs on a non-Tokio thread (native UI toolkits, `ashpd`,
`accesskit_unix`). `scripts/check-app-deps.sh` enforces it with all workspace
features enabled (YAN-2475, ADR-0002 § Negative).

## MSRV

`rust-version` in `[workspace.package]` is the MSRV and matches the toolchain
pinned in [`rust-toolchain.toml`](../../rust-toolchain.toml). Bump them in one PR,
together with every pinned `toolchain:` in `.github/workflows/` (`lint.yml`,
`deps.yml`, `copilot-setup-steps.yml`). Workflows that ask for `stable` pick up
`rust-toolchain.toml` instead. The MSRV must
stay at or above the MSRV of the pinned UI toolkit revision.

## Adding a dependency

Before adding a crate, the PR description answers:

1. **Why:** what it does that the standard library or an existing dependency
   cannot. Check `cargo tree` for something already in the graph first.
2. **Cost:** how many new crates it adds (`cargo tree -e normal` before and after),
   its license, and whether it is maintained (recent releases, open RustSec
   advisories).
3. **Placement:** which crate takes it. `crosshook-view-model` takes no tokio, I/O
   or UI toolkit (`scripts/check-app-deps.sh`), and `crosshook-app` takes no UI
   toolkit.

`cargo deny check` must pass. A new license or source exception goes into
`deny.toml` in the same PR, with a reason.

## UI toolkit revision

The native UI toolkit is pinned to a git revision (`rev = "<sha>"`, never a branch).
The crate that first adds it also adds the upstream URL to `allow-git` in
`deny.toml`, once the framework ADR has chosen the toolkit.

- **Scheduled bumps:** once a quarter, in January, April, July and October.
- **Out-of-cycle bumps:** only for security or crash fixes.
- **Checklist for every bump:**
  1. Read the upstream changes between the old and new revision.
  2. Run the native UI simulator and snapshot suite.
  3. Run the benchmark gate (`docs/internal/bench/`).
  4. Run the real-hardware perf checkpoint.
  5. Record the new revision, and why, in the framework ADR.
- **Releases:** a release never moves to a new toolkit revision without a bench run
  on that revision.

## Review cadence

- **Every manifest or lockfile change:** the `deps.yml` workflow runs `cargo deny check`.
- **Weekly:** `deps.yml` runs on a schedule to catch newly published advisories.
  Fix a new advisory with `cargo update -p <crate>` where a compatible fix exists.
  Otherwise add an `ignore` entry with a reason and file a follow-up issue.
- **Yearly (January):** set `unmaintained = "all"` locally, review every
  transitive unmaintained crate, and re-check that each `ignore` and license
  exception is still needed.
