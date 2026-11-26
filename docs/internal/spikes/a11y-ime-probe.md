# AccessKit, Orca live regions and IME probe (M0-09)

Probe for [YAN-877](https://linear.app/itcn/issue/YAN-877) / GitHub [#658](https://github.com/yandy-r/crosshook/issues/658). It feeds the [M0-07 native UI framework decision](https://linear.app/itcn/issue/YAN-860) alongside the [iced spike](iced.md) (M0-04) and the [gpui spike](gpui.md) (M0-05).

Probe code is throwaway. It lives on two spike branches that are never merged to `main`:

- [`spike/yan-877-a11y-gpui`](https://github.com/yandy-r/crosshook/tree/spike/yan-877-a11y-gpui) (head `38a0c0b2`, on `spike/gpui`): a live-region announcer, a stderr probe of the search field's text, and a startup hook that focuses the search field.
- [`spike/yan-877-a11y-iced`](https://github.com/yandy-r/crosshook/tree/spike/yan-877-a11y-iced) (head `4c9937e9`, on `spike/iced`): the same text probe and focus hook.

This report and the [redacted text captures](a11y-ime-probe/evidence/) are the only artifacts intended for `main`.

This is evidence, not a decision.

## Findings for M0-07

- **gpui has an upstream AccessKit integration, but does not pass gate 8.** Orca names buttons; several form controls are unnamed or not announced, and the modal, grid cards, and log lack accessible nodes. The live-region probe announces on AT-SPI activation but does not yet establish focus-preserving announcements.
- **iced has no viable AccessKit path at the tested pin.** The registry probe did not find the upstream spike by name, the pop-os fork needs API and adapter work, and the draft PR does not resolve dependencies.
- **Both spikes committed Japanese text in all 12 desktop test cells.** These cover fcitx5 and ibus under nested sway, host XWayland, and nested gamescope XWayland. This is not a pass for gate 9: Steam Deck Game Mode and the Steam keyboard were not tested.

## Environment

E1, as in [`../bench/environments.md`](../bench/environments.md): Fedora 45, KDE Plasma 6 Wayland (KWin 6.8.2), NVIDIA RTX 5070 Laptop, driver 620.70.06. Orca 51.1, AT-SPI 2.62.1, fcitx5 5.1.23 with Mozc, ibus 1.5.35 with Anthy, gamescope from Fedora 45, sway (installed for this probe). The spikes ran against the seed-42 bench fixtures (500 profiles) in isolated XDG roots.

## Accessibility: names and roles (Orca)

Method: queried the AT-SPI tree with pyatspi, requested focus on each control and compared the result with Orca’s `SPEECH OUTPUT`. Switched pages through the AT-SPI `click` action. The captures omit unrelated desktop data.

### gpui

| Control                 | Kit widget          | AT-SPI role / name                                                    | State and value                                                                                    | Orca speech                                                                               | Result      |
| ----------------------- | ------------------- | --------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- | ----------- |
| Button                  | `Button`            | `button` `'Library'`, `'Save'`, …                                     | focusable, `click` action works                                                                    | "Library button", "Save button"                                                           | **Pass**    |
| Text input              | `Input`             | `entry` `'Search games'` (placeholder used as name)                   | editable. **No AT-SPI Text interface** (`queryText` not implemented), so the value is not readable | announced only the window frame                                                           | **Partial** |
| Toggle / switch         | `Switch`            | `toggle button`; named only when the switch has its own label         | `pressed` follows the state (checked through AT-SPI `click`, confirmed on screen)                  | "toggle button pressed", "Disable Steam overlay (optimization) toggle button not pressed" | **Partial** |
| Checkbox                | not in the slice    | —                                                                     | —                                                                                                  | —                                                                                         | not tested  |
| Select / pick list      | `Select`            | `combo box`, **no name**                                              | no value, no children while closed                                                                 | "combo box"                                                                               | **Partial** |
| Number input            | `NumberInput`       | `spin button`, **no name**; child `entry` named after the placeholder | no AT-SPI Value                                                                                    | "spin button"                                                                             | **Partial** |
| Modal dialog            | `Dialog` (Kit Root) | **no node**: only the dialog's children (entry, `Close`) appear       | not modal                                                                                          | children only                                                                             | **Fail**    |
| Grid card               | custom `div`        | **no node**                                                           | —                                                                                                  | —                                                                                         | **Fail**    |
| Log region (console)    | custom ring view    | **no node**                                                           | —                                                                                                  | —                                                                                         | **Fail**    |
| Live region (polite)    | probe `Announcer`   | `status bar` with a `label` child per message                         | `Live::Polite`                                                                                     | "Launch started" on AT-SPI click; button loses focus                                      | **Partial** |
| Live region (assertive) | probe `Announcer`   | `notification` with a `label` child per message                       | `Live::Assertive`                                                                                  | "Launch failed: trainer not found" on AT-SPI click; button loses focus                    | **Partial** |

Evidence: [AT-SPI trees](a11y-ime-probe/evidence/gpui-trees.txt), [Orca and interface results](a11y-ime-probe/evidence/gpui-orca.txt), and [live-region activation](a11y-ime-probe/evidence/gpui-live-region.txt).

Notes:

- The slice's own widgets (cover cards, the log view) are plain `div`s with no `.role()`. gpui drops nodes with no role, so they never reach AT-SPI. That is app code: [`Element::a11y_role`](https://docs.rs/gpui-pre) plus `aria_*` setters and synthetic children cover them, as the announcer shows.
- Unnamed switches, selects and spin buttons: the spike labels them through Kit's form `field().label(…)`, which draws a caption but does not set `aria_label` on the control. Kit has the setters (`accessibility_label`). The fix is to pass the label through, in the app or upstream.
- The missing dialog node is a known Kit bug. GPUI Kit [#3595](https://github.com/longbridge/gpui-kit/issues/3595) says `FocusTrapContainer` dropped the role in 0.7.x and that it is fixed on `main`, but the dialog still has no name and is not marked modal.
- Live regions need no Kit support: `role(Role::Status|Alert)` plus `set_live` on a node whose child id changes per message. Kit later added a `LiveRegion` (GPUI Kit [#4094](https://github.com/longbridge/gpui-kit/issues/4094)).

### iced, upstream (pinned `a8231fd9`)

The registry query for `crosshook-ui-spike-iced` found no matching application ([registry captures](a11y-ime-probe/evidence/iced-upstream-registry.txt)). The captures do not rule out registration under another name. Individual controls were not assessed. Upstream tracking issue: iced-rs/iced [#552](https://github.com/iced-rs/iced/issues/552), open since 2020.

### iced path (a): `pop-os/iced` fork with `a11y`

Fork head `115018bf` (2026-11-10). It is 289 commits ahead of and 787 behind upstream `master`. The fork diverged at `b11ad9ff` (2025-12-07), so it is older than our pin, which is on upstream `master`.

1. **Compile with our pin.** Swapping the spike's iced dependency to the fork with `a11y` gave 16 errors after a `bitflags` lock bump: `iced::Widget` and `widget::Meta` are missing, `operation::scrollable` and `Animation` are missing, and `Widget::update`, `layout`, `operate` and `overlay` have different signatures. See the [build excerpts](a11y-ime-probe/evidence/iced-build-excerpts.txt). The spike's custom widgets (focus wrapper, log view) would have to be rewritten against the fork's 0.14 API. Patch size: the fork is 312 files, +31,697/−3,914 lines over the merge base (107 files, +19,609/−1,446 in `accessibility`, `core/src/widget`, `winit/src`, `widget/src`).
2. **Does the fork expose anything?** Its own `tour` example, built with `--features iced/a11y,iced/tokio`, was checked on E1 (KWin Wayland and XWayland):
   - **Not registered.** The fork's AccessKit (`wash2/accesskit` rev `6c20249`) waits for the `ScreenReaderEnabled` property on `org.a11y.Status`. KDE's AT-SPI bus only has `IsEnabled`, so the adapter never activates. Upstream AccessKit fixed this in [AccessKit #715](https://github.com/AccessKit/accesskit/pull/715) (watch `IsEnabled`), merged 2026-05-23. The fork still pins the old revision.
   - **Window only.** With that one line patched locally (`receive_is_enabled_changed`), `tour` registered in the exploratory run (raw capture not retained), but the tree is `application 'tour'` → `frame ''` with no children. In `winit/src/lib.rs` `a11y_enabled` is set and never read: nothing pushes widget nodes to the adapter. The last `adapter.update_if_active(...)` call was removed in fork commit `93bc4bbd` ("feat: subsurfaces", 2025-03-14), and the "a11y refactor" (`b4346ff3`, 2026-02-04) did not restore it.
   - Widget coverage in the fork: `a11y_nodes` exists for button, checkbox, column, container, image, mouse_area, row, scrollable, slider, svg, toggler and text. There is none for `text_input`, `pick_list` or a modal. For text input specifically, see libcosmic [#1429](https://github.com/pop-os/libcosmic/issues/1429).
   - `pop-os/iced` has issues disabled, so there is nowhere upstream to file the fork bugs.
3. **Merge outlook:** none. Upstream has no AccessKit, and the fork is a downstream of COSMIC with its own API.

Result: **Fail** on every control, on both E1 window systems.

### iced path (b): PR #1849

Draft PR [iced-rs/iced#1849](https://github.com/iced-rs/iced/pull/1849) ("WIP: Iced accessibility", head `pop-os:iced-accessibility` at `bf0deae0`, last commit 2023-05-12). It is based on iced 0.9, 3,956 commits behind upstream `master`, and GitHub reports it `CONFLICTING`; a local `git merge-tree` also reported conflicts.

Pointing the spike at the PR head does not resolve dependencies. The `crisp` feature does not exist in iced 0.9, and the PR's `iced_winit` depends on `pop-os/winit` branch `iced`, which has been deleted (`cannot locate remote-tracking branch 'origin/iced'`). Result: **does not compile**, not because of the spike code but because the PR's dependency graph is gone.

### Live-region announcer

The issue asks for one `Announcer` per spike.

- **gpui:** [`announcer.rs`](https://github.com/yandy-r/crosshook/blob/spike/yan-877-a11y-gpui/src/crosshook-native/crates/crosshook-ui-spike-gpui/src/announcer.rs) keeps two zero-size nodes in every frame: `Role::Status` + `Live::Polite` and `Role::Alert` + `Live::Assertive`. Each message is a synthetic `Role::Label` child whose id is a sequence number. AT-SPI click produced both expected utterances and the corresponding live-region children; the plain button did neither. The probe buttons lost accessible focus on click. In the recorded Enter run neither region changed and Orca repeated the stale error even for the plain button. **Partial**: speech works on AT-SPI click, but focus preservation and keyboard activation are not established.
- **iced:** **not buildable**. Upstream has no accessibility tree, and the fork pushes none, so no live node can reach AT-SPI. A prototype would first need the fork's adapter wiring repaired (see path (a)).

## IME

Started each fixture-backed spike with `SPIKE_FOCUS_SEARCH=1`, selected the Japanese engine, then typed `nihongo`, Space (convert), and Enter (commit). The criterion below is the converted string in the app’s `SPIKE_SEARCH_TEXT` log. The runs do not establish candidate-window positioning across the matrix.

| Environment                        | Engine         | gpui commit | iced commit | Evidence                                                       |
| ---------------------------------- | -------------- | ----------- | ----------- | -------------------------------------------------------------- |
| Nested sway, Wayland text-input-v3 | fcitx5 + Mozc  | `ニホンゴ`  | `ニホンゴ`  | [Wayland log](a11y-ime-probe/evidence/ime-wayland-sway.txt)    |
| Nested sway, Wayland text-input-v3 | ibus + Anthy   | `日本語`    | `日本語`    | [Wayland log](a11y-ime-probe/evidence/ime-wayland-sway.txt)    |
| Host KWin XWayland, XIM            | fcitx5 + Mozc  | `ニホンゴ`  | `ニホンゴ`  | [XWayland log](a11y-ime-probe/evidence/ime-x11-xim.txt)        |
| Host KWin XWayland, XIM            | ibus + Anthy   | `日本語`    | `日本語`    | [XWayland log](a11y-ime-probe/evidence/ime-x11-xim.txt)        |
| Nested gamescope XWayland, XIM     | fcitx5 + Mozc  | `ニホンゴ`  | `ニホンゴ`  | [gamescope log](a11y-ime-probe/evidence/ime-gamescope-xim.txt) |
| Nested gamescope XWayland, XIM     | ibus + Anthy   | `日本語`    | `日本語`    | [gamescope log](a11y-ime-probe/evidence/ime-gamescope-xim.txt) |
| Steam Deck Game Mode, Steam+X      | Steam keyboard | Not tested  | Not tested  | Hardware unavailable                                           |

Neither toolkit hit the gpui XIM bugs the issue lists in our runs: zed [#58192](https://github.com/zed-industries/zed/issues/58192) and [#54959](https://github.com/zed-industries/zed/issues/54959) are closed, and [#60578](https://github.com/zed-industries/zed/issues/60578) (IME breaks for good after an `XIMClientError`, no reconnect) is still open. It did not trigger here, but no fault was injected to test it.

### Wayland IME on the host KWin session

The host session uses `plasma-keyboard` as its input method, so on the host KWin the spikes get no CJK engine. Neither does any other client. Switching KWin's input method takes System Settings or a re-login, which the probe did not do to the owner's live session. The Wayland rows therefore ran in a nested sway with fcitx5 or ibus as its `input-method-v2` server. A nested KWin with `--inputmethod` was tried first; its input method attached to the host display, so the controls (kdialog, zenity) also got ASCII and the rig was dropped.

## What was not measured

The available evidence does not cover these checks:

- **E2-D (Deck Desktop, KDE) Orca run** and **E2-G Game Mode Steam OSK (Steam+X).** The maintainer deferred these checks because no Steam Deck is available ([`environments.md`](../bench/environments.md)). Nested gamescope tests desktop IME through XIM, not Steam’s on-screen keyboard. See [YAN-846](https://linear.app/itcn/issue/YAN-846) for the rest of the E2 matrix.
- **Orca on E1 GNOME Wayland.** E1 runs KDE Plasma, so the Orca rows are KDE (KWin Wayland), the same desktop as E2-D.
- **IME preedit and candidate placement.** The 12 desktop cells verify committed text only; they do not measure preedit display or candidate positioning.
- **Screen captures per failing case.** The AT-SPI dumps and the Orca speech log are text evidence that can be diffed; the screenshots used to read the IME candidates are not committed (they show the owner's desktop).

## Failures, upstream issues and mitigation estimates

| Failure                                                                    | Upstream                                                                                                                                                                 | Mitigation                                                                                                 | Estimate                           |
| -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------- | ---------------------------------- |
| iced: no AccessKit at all                                                  | iced-rs/iced [#552](https://github.com/iced-rs/iced/issues/552); [#1849](https://github.com/iced-rs/iced/pull/1849) stale, does not build                                | Write an AccessKit layer ourselves: adapter wiring, a node per widget, live regions, focus sync            | 13 (own fork; ongoing rebase cost) |
| iced fork: AT-SPI activation keyed on `ScreenReaderEnabled` (KDE has none) | fixed upstream in AccessKit [#715](https://github.com/AccessKit/accesskit/pull/715); `pop-os/iced` has issues disabled                                                   | Bump the fork's `accesskit` pin                                                                            | 1                                  |
| iced fork: widget tree never sent to the adapter                           | none filable (`pop-os/iced` issues disabled); regression in fork commit `93bc4bbd`                                                                                       | Restore `update_if_active` with the tree from `a11y_nodes`                                                 | 3                                  |
| iced fork: no nodes for `text_input`, `pick_list`, modal                   | pop-os/libcosmic [#1429](https://github.com/pop-os/libcosmic/issues/1429)                                                                                                | Implement `a11y_nodes` for each                                                                            | 3                                  |
| iced: our pin cannot move to the fork (16 API errors)                      | —                                                                                                                                                                        | Port the spike's custom widgets to the fork's 0.14 API                                                     | 3                                  |
| gpui Kit: modal dialog has no node, no name, not modal                     | GPUI Kit [#3595](https://github.com/longbridge/gpui-kit/issues/3595) (role fixed on `main`, name and modal open)                                                         | Take the Kit fix, or wrap the dialog body in our own `Role::Dialog` node with `aria_label` and `set_modal` | 1                                  |
| gpui Kit: form field caption does not name its control                     | related: GPUI Kit [#3662](https://github.com/longbridge/gpui-kit/issues/3662); fixed for the setting page in [#3582](https://github.com/longbridge/gpui-kit/issues/3582) | Pass the label through `accessibility_label` in our form primitive (M1-17)                                 | 1                                  |
| gpui Kit: text input has no AT-SPI Text interface                          | fixed in Kit [#3818](https://github.com/longbridge/gpui-kit/issues/3818) (single-line) and [#3894](https://github.com/longbridge/gpui-kit/issues/3894) (multi-line)      | Take a Kit release with the fix                                                                            | 1                                  |
| gpui: custom widgets (cards, log) have no nodes                            | — (app code)                                                                                                                                                             | `role()` + `aria_*` on cards; `Role::Log` with synthetic rows for the visible window of the log            | 2                                  |
| gpui: IME may break for good after an XIM error                            | zed [#60578](https://github.com/zed-industries/zed/issues/60578)                                                                                                         | Watch it; fault-inject in the M0-06 matrix                                                                 | 1                                  |

Totals for parity with the slice: **gpui about 6 points**, mostly app-side and Kit pass-throughs. For iced, the fork repair is about **10 points**; building our own integration against upstream is a separate **13-point** option. Neither estimate includes ongoing maintenance. The gpui estimate excludes the Kit upgrade itself.

## Persistence

None. This is a probe; the results are docs only. No TOML settings keys, no SQLite schema, no runtime state outside the throwaway spike branches.
