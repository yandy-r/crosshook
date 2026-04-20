# Protontricks Flatpak as a Prefix Dependency Path — Viability Note

> Closes Issue [#275] and deep-research Phase 3 task 3.4 in
> [`14-recommendations.md`](./14-recommendations.md) / AO-3 in
> [`13-opportunities.md`](./13-opportunities.md) / Evidence Gap #5 in
> [`10-evidence.md`](./10-evidence.md).
>
> This document answers — in writing — whether CrossHook, when distributed as
> a Flatpak, should treat the **Protontricks Flatpak**
> (`com.github.Matoking.protontricks`) as a supported bridge for prefix
> dependency management, so that future Flatpak work does not reopen the
> question from scratch. Acceptance criteria from #275 are answered in §5.
>
> Verification scope: **desk research only** (per user direction). No local
> `flatpak run` smoke tests were performed; any claim that depends on
> unverified local behaviour is explicitly flagged in §2.
>
> All line numbers below are from the tree at commit `main` ≈ 2026-04-20.

[#275]: https://github.com/yandy-r/crosshook/issues/275
[#276]: https://github.com/yandy-r/crosshook/issues/276

## TL;DR

- **Verdict: not viable as a first-class supported dependency path.** Upstream
  Protontricks has four open, load-bearing sandbox limitations that block
  CrossHook from relying on `com.github.Matoking.protontricks` to inspect
  Proton installs, enumerate the Steam library, honour `STEAM_COMPAT_*`, or
  reach custom prefix paths. CrossHook cannot promise these will work for a
  Flatpak-deployed user even if the Flatpak is installed.
- **Viable only as a user-discretion fallback, which is already surfaced.**
  `src/crosshook-native/assets/default_host_readiness_catalog.toml:363, 388`
  already mentions `com.github.Matoking.protontricks` as an _alternative_ for
  `GamingImmutable` and `Nobara` distro families. That's the correct level of
  commitment; no promotion is warranted.
- **Separately, `prefix_deps/` is already not Flatpak-aware.** Detection and
  invocation use sandbox-local `Command::new` / `PATH` rather than the
  `platform.rs` host gateway (see §1.4). That is a pre-existing gap — not
  new to #275 — but any future promotion of the Flatpak path would have
  to fix it first. Out of scope for this note; flagged for follow-up.

---

## §1 — What CrossHook already assumes about winetricks and protontricks

### 1.1 Detection chain

`src/crosshook-native/crates/crosshook-core/src/prefix_deps/detection.rs:22-88`
implements a fixed 3-tier priority:

```rust
// Priority 1: Settings override
// Priority 2: `winetricks` on PATH
// Priority 3: `protontricks` on PATH
```

Winetricks is preferred over Protontricks; Protontricks is the fallback. A
user-supplied absolute path in settings overrides both. The two public
helpers `resolve_winetricks_path()` and `resolve_protontricks_path()`
(`detection.rs:82-89`) both resolve via `PATH`.

### 1.2 Invocation surface

`src/crosshook-native/crates/crosshook-core/src/prefix_deps/runner.rs:81-101`
invokes the detected binary:

```rust
let mut cmd = Command::new(binary_path);
if matches!(tool_type, PrefixDepsTool::Protontricks) {
    let app_id = steam_app_id.ok_or_else(…)?;
    cmd.arg(app_id);
}
cmd.arg("list-installed");
cmd.env("WINEPREFIX", &resolved_prefix);
apply_host_environment(&mut cmd);
```

Two things to note:

- The constructor is `tokio::process::Command::new`, **not**
  `platform::host_command`.
- `apply_host_environment`
  (`src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers/environment.rs:165-169`)
  copies the captured host env _into the Command_, which then relies on
  normal process-env inheritance. Under Flatpak, env vars set via `.env()`
  on a raw `Command` are silently dropped unless the command is also wrapped
  by `flatpak-spawn --host` (see `platform/gateway.rs:20-31` for the contract
  warning). This is what feeds §1.4 below.

### 1.3 Host-tool gateway (ADR-0001) — the Flatpak-aware path

`src/crosshook-native/crates/crosshook-core/src/platform/gateway.rs:32-86`
defines `host_command` / `host_command_with_env` /
`host_command_with_env_and_directory`. Inside Flatpak they wrap the program
with `flatpak-spawn --host`; outside Flatpak they behave as ordinary
`Command::new`. `host_command_exists`
(`platform/gateway.rs:325-345`) routes `which <binary>` through
`host_std_command` when `is_flatpak()` is true, so host-tool probes give the
right answer from inside a Flatpak sandbox.

`protontricks` is on the gateway denylist per ADR-0001 — see
[`docs/architecture/adr-0001-platform-host-gateway.md`](../../architecture/adr-0001-platform-host-gateway.md)
and the check in `scripts/check-host-gateway.sh`. So **any in-sandbox
invocation of protontricks is required by policy to go through the
gateway**, whether the binary is a host-native protontricks or a
`flatpak run com.github.Matoking.protontricks` shell-out.

### 1.4 Pre-existing gap — `prefix_deps/` is not Flatpak-aware

A grep of the module confirms zero references to the gateway:

```text
$ rg -n 'host_command|flatpak-spawn|is_flatpak' src/crosshook-native/crates/crosshook-core/src/prefix_deps
(no matches)
```

`detection.rs` walks the sandbox's own `PATH` via `env::split_paths`;
`runner.rs` spawns `tokio::process::Command::new(binary_path)` directly.
Inside a CrossHook Flatpak build, these two code paths operate on the
sandbox's filesystem view, not the host's. Unless the sandbox happens to
have `winetricks` / `protontricks` bundled (it does not), both detection
and invocation fail quietly — independent of any Flathub Protontricks
question.

This note records the gap but **does not widen #275's scope to fix it**.
Any future work to promote the Flatpak-Protontricks path would need to
route the module through `host_command` / `host_command_with_env` first,
which in turn would need to thread `WINEPREFIX` / `STEAM_COMPAT_*` through
the `--env=KEY=VALUE` argv (see the warning at `platform/gateway.rs:20-31`).

### 1.5 Catalog entries and install advice

`src/crosshook-native/assets/default_host_readiness_catalog.toml:290-398`
defines the readiness-catalog tool entries:

- `[[tool]] tool_id = "winetricks"` (lines 290-344) with per-distro
  `[[tool.install]]` rows for Arch, Nobara, Fedora, Debian, Nix,
  SteamOS, GamingImmutable, BareImmutable, Unknown.
- `[[tool]] tool_id = "protontricks"` (lines 346-398) with the same
  families. Notably, lines **363** and **388** both already mention
  `com.github.Matoking.protontricks` as an alternative, and line 383
  lists "Flatpak" alongside pip and distro packages for SteamOS.

`src/crosshook-native/crates/crosshook-core/src/onboarding/install_advice.rs`
consumes those entries to surface install suggestions. For the answer
framing in §5 below, the key observation is that **the Flathub Protontricks
path is already in the catalog as an alternative**, so the policy question
is really "should #275's outcome be _promote it_ or _leave it where it
is_?". The evidence in §3 answers _leave it where it is_.

### 1.6 Research anchors — §1

- [`10-evidence.md`](./10-evidence.md) §Gap 5 — explicit research gap.
- [`13-opportunities.md`](./13-opportunities.md) §AO-3 lines 365-384 —
  architecture opportunity framing.
- [`14-recommendations.md`](./14-recommendations.md) §3 Phase 3 row 3.4
  line 121 — deferred Phase 3 investigation task.

---

## §2 — Can CrossHook call the Protontricks Flatpak at all?

### 2.1 The invocation shape

From a CrossHook Flatpak sandbox, the only way to reach another Flatpak is
through the existing `--talk-name=org.freedesktop.Flatpak` grant
(`packaging/flatpak/dev.crosshook.CrossHook.yml:32`; also verified in
`15-gamemode-and-background-ground-truth.md` §3.1). The call shape is:

```text
flatpak-spawn --host flatpak run com.github.Matoking.protontricks <args…>
```

Routed through `platform::host_command` in line with ADR-0001, the Rust
call site would be:

```rust
let mut cmd = platform::host_command("flatpak");
cmd.arg("run").arg("com.github.Matoking.protontricks");
cmd.arg(app_id).arg("list-installed");
```

No additional Flatpak manifest permissions are required — `flatpak-spawn
--host` is already authorised, and `flatpak run` at the host level inherits
the host user's Flatpak session.

### 2.2 Upstream confirmation — not available

A search of the Protontricks upstream (`Matoking/protontricks`), the
Flathub packaging (`flathub/com.github.Matoking.protontricks`), and the
Flathub docs did not surface a documented pattern for invoking the
Protontricks Flatpak _from within another Flatpak sandbox_ via
`flatpak-spawn --host flatpak run …`. Lutris and Heroic are the closest
analogues — both run on the host and invoke Protontricks directly, not
through a double-wrap — so their precedent does not transfer.

**Conclusion**: the invocation shape is _plausible_ but **unverified**
against real behaviour. A hands-on smoke test is required before any
promotion beyond "alternative". The evidence in §3 below means that a
smoke test alone would not change the verdict in most cases — the
blockers are at the Protontricks sandbox level, not the invocation
nesting level.

---

## §3 — Why it fails the first-class bar (upstream evidence)

These are open upstream issues at the time of writing. Each is a
stand-alone blocker for CrossHook's dependency-management use case.

### 3.1 Cannot detect Proton from Flatpak Steam

[Matoking/protontricks#446](https://github.com/Matoking/protontricks/issues/446).
With Flatpak Steam installed, Flatpak Protontricks fails with
`Could not find configured Proton installation!` even when Proton
Experimental is selected as the default tool. A subset of users see it fall
back to stable Proton if present in standard locations; the rest cannot use
the tool at all. This is the load-bearing blocker — Proton version detection
is a prerequisite for nearly every prefix operation CrossHook would
delegate to Protontricks.

### 3.2 Cannot read Steam library metadata from the sandbox

[Matoking/protontricks#434](https://github.com/Matoking/protontricks/issues/434).
Even when Steam directories are visible via filesystem overrides, Flatpak
Protontricks cannot reach some of Steam's internal configuration. Library
enumeration fails, which defeats both interactive (`--gui`) and scripted
(`--appid`) modes.

### 3.3 Custom env vars not visible inside the Flatpak sandbox

[Matoking/protontricks#327](https://github.com/Matoking/protontricks/issues/327).
`WINETRICKS` is explicitly called out; the same class of restriction
applies to `STEAM_COMPAT_CLIENT_INSTALL_PATH`,
`STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_MOUNTS`, and any custom
`PROTON_*` override. CrossHook sets these routinely to point Proton at
non-default prefixes (`src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers/environment.rs`),
so losing them at the Flatpak boundary would produce silently wrong
behaviour, not a clean error.

### 3.4 Colon-in-path `flatpak override` bug — Windows drive letters

[flathub/com.github.Matoking.protontricks#27](https://github.com/flathub/com.github.Matoking.protontricks/issues/27).
`flatpak override --user --filesystem=<path>` parses the argument with `:`
as a delimiter, so any prefix path containing a Windows drive letter
(`c:/…`, `z:/…`) breaks. This is a common shape on Steam Deck and for
anyone who has nudged a Proton prefix's drive mapping.

### 3.5 Default filesystem permissions are minimal

The Flathub manifest grants Flatpak Protontricks access only to the
standard Steam install paths (`~/.steam`, `~/.local/share/Steam`). Any
other Steam library — including the Steam Deck SD card, secondary SSDs,
or a user-configured custom library — requires either Flatseal or a
`flatpak override --user --filesystem=…` step per library. CrossHook
cannot rely on end users having configured these; discovering they _have
not_ requires running an invocation that then fails. This is a UX
regression vs. host-native protontricks, which inherits the user's
filesystem view by default.

### 3.6 Trust signals

The Flathub listing is marked "Potentially unsafe" and "Desktop Only",
x86_64 only, community-maintained (not upstream Valve/Wine), and shipped
via a single stable channel with no beta. None of this is disqualifying on
its own, but it means "we recommend installing this Flatpak as part of
onboarding" carries a non-trivial endorsement weight that should match
the tool's actual reliability inside CrossHook's use case. Today it does
not.

---

## §4 — The narrow green path

Flatpak Protontricks is not useless. It works for the intersection of:

- Steam installed at a default library location (no SD card, no external
  SSD, no `libraryfolders.vdf` with custom roots).
- No custom `STEAM_COMPAT_*` remapping — the user accepts Protontricks'
  default Proton discovery.
- No non-ASCII or colon-bearing prefix paths.
- The user is willing to run `flatpak override --user --filesystem=<path>`
  or Flatseal once per additional path.
- The user is _not_ running Flatpak Steam (else §3.1 bites).

This is a meaningful subset of the Linux gaming population — particularly
traditional-distro desktop users on a single primary drive. It is **not**
the CrossHook core audience: Steam Deck power users, Flatpak-Steam users,
and anyone with a custom library layout all fail out of this set.

The pragmatic conclusion: keep Flatpak Protontricks available as an
"if it happens to work for your setup" alternative (already so in the
catalog TOML), but do not bias the onboarding experience toward it.

---

## §5 — Answer to Issue #275 acceptance criteria

The issue body lists three acceptance criteria. Each is answered here
verbatim:

> **The repo has a concrete answer on whether the Protontricks Flatpak is
> a viable path for CrossHook.**

**Answered.** _Not viable as a supported dependency-management path._
_Viable only as a user-discretion fallback, already surfaced in
`default_host_readiness_catalog.toml:363, 388` as an alternative for
`GamingImmutable` and `Nobara` families._ See §3 for the blocker list,
§4 for the narrow green path, §1.5 for where this lives in-repo today.

> **If viable, the issue documents the required invocation and UX
> constraints.**

**Partially applicable.** For the narrow green path documented in §4,
the required invocation is
`flatpak-spawn --host flatpak run com.github.Matoking.protontricks …`
via `platform::host_command` (§2.1). The UX constraints are: the user
must have already run `flatpak install flathub
com.github.Matoking.protontricks`; must have granted Flatseal /
`flatpak override` access for any non-default Steam library path; and
must not be a Flatpak-Steam user. Because §3 still applies inside that
narrow set, CrossHook should not promote this path — the catalog
"alternative" framing is correct.

> **If not viable, the issue documents why, so future Flatpak work does
> not reopen the question from scratch.**

**Answered.** §3 enumerates four upstream issues
(Matoking#446, #434, #327, flathub #27) plus the default-permissions
gap and the trust signals. The verdict is time-scoped: if upstream
resolves #446 (Proton detection from Flatpak Steam) the question should
be reopened, because #446 is the load-bearing blocker for CrossHook's
use case and everything else in §3 is survivable.

---

## §6 — Recommendation & closure path

### 6.1 No net-new code required to close #275

This document _is_ the deliverable. The issue body pre-negotiated
this — "Investigation notes and invocation experiments belong in repo
docs or issue text rather than app storage." Merge this file, close
#275 referencing this doc.

### 6.2 Optional, strictly separate follow-ups

None of these are required to close #275; each should be a separate
issue so the decision recorded here stays crisp.

- **Minor polish on the catalog TOML** — annotate the Protontricks
  `alternatives` strings at `default_host_readiness_catalog.toml:363, 383,
388` to reference this doc so users hit the caveats before installing.
  Low effort, no schema change.
- **Fix the pre-existing `prefix_deps/` Flatpak gap** (§1.4). Route
  `detection.rs` and `runner.rs` through `platform::host_command_exists`
  and `platform::host_command_with_env`. Independent of #275, but
  _required_ before any future promotion of the Flatpak Protontricks
  path. File as a separate "area:compatibility" issue against #276.
- **Re-open this question if upstream resolves
  [Matoking/protontricks#446](https://github.com/Matoking/protontricks/issues/446)**.
  Document-owner note: #446 is the load-bearing blocker; its closure is
  the trigger that should bring the viability question back for review.

### 6.3 Closure path to parent tracker [#276]

The research tracker [#276] can mark the following items closed by this
work:

- Phase 3 task 3.4 — "Protontricks-as-Flatpak investigation"
  ([`14-recommendations.md`](./14-recommendations.md) line 121) —
  answered in §5. Outcome: do not promote; keep existing catalog
  alternative framing.
- AO-3 — "Investigate Protontricks-as-Flatpak for prefix dependency
  management" ([`13-opportunities.md`](./13-opportunities.md) lines
  365-384) — answered in §5. Outcome: opportunity declined with
  documented reasoning.
- Evidence Gap #5 — "Protontricks as an Alternative to Winetricks"
  ([`10-evidence.md`](./10-evidence.md) line 336) — closed by §3/§4.

No Phase 1 / Phase 2 items are affected by this decision.
