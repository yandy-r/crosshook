## Executive Summary

The most pragmatic implementation is to add a dedicated install sub-tab inside the existing Profile panel, back it with a small install orchestration layer in `crosshook-core`, and output a standard `proton_run` profile when setup is complete. This keeps the feature close to the current architecture: reuse Proton discovery, reuse profile persistence, reuse Proton runtime environment assembly, and add only the missing workflow pieces around installer execution and post-install executable confirmation.

The primary recommendation is to avoid a hard dependency on `umu-run` in v1 even though the existing shell script uses it. CrossHook already launches Proton directly, and the install feature becomes materially easier to ship, package, and support if v1 stays on that direct path while preserving room for optional `umu-run` integration later if real compatibility data shows it is needed.

### Recommended Implementation Strategy

- Recommended strategy:
  - Build the feature as a new install workflow, not as a hidden mode inside existing launch commands
  - Keep the final artifact as a normal saved profile so launch, export, and future profile management continue to work without special cases
  - Reuse existing Proton detection and editable Proton-path controls exactly where possible

- Phasing:
  1. Phase 1 - Foundation
     - Add install-domain models, validation, default prefix derivation, and profile-generation helpers in `crosshook-core`
     - Extract shared Proton environment assembly from the current launch runner so install and launch stay consistent
  2. Phase 2 - Core UX and Tauri wiring
     - Add `commands/install.rs`
     - Add the install sub-tab UI and hook it into `ProfileEditor`
     - Launch installer media, stream logs/status, and preserve draft state during retries
  3. Phase 3 - Finalization and polish
     - Add post-install executable discovery/confirmation
     - Add non-empty-prefix warning and overwrite-name handling
     - Tighten copy, inline help, and error states

- Scope control:
  - v1 should support one installer executable, one prefix, one selected Proton version, and an optional trainer
  - v1 should explicitly defer advanced per-game environment overrides, store-specific metadata, or `winetricks`-style post-install automation

### Technology Decisions

| Decision                           | Recommendation                                             | Rationale                                                                                |
| ---------------------------------- | ---------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| Install execution path             | Use direct `proton run` through native Rust/Tauri commands | Reuses current runtime model and avoids introducing a new system dependency immediately  |
| `umu-run` support                  | Defer to optional future enhancement                       | Good compatibility reference, but a hard dependency increases packaging and support cost |
| Draft persistence                  | Add a lightweight install draft model                      | Prevents users from losing setup data after failures or interruption                     |
| Final saved artifact               | Keep standard `GameProfile` TOML                           | Avoids downstream special cases in launch, export, or community flows                    |
| Default prefix path                | `~/.config/crosshook/prefixes/<profile-slug>`              | Predictable, app-owned, and aligned with the user's requested default                    |
| Executable selection after install | Require explicit confirmation, assisted by prefix scanning | Prevents the most damaging user error: saving the installer as the game path             |
| Proton selection UX                | Keep dropdown plus editable path field                     | Matches the existing lesson and current UI behavior for detected installs                |

### Quick Wins

- Add a derived default prefix preview immediately after the profile name field so users understand where the install will land before they click anything.
- Reuse the existing `ProtonPathField` pattern instead of building a new Proton selector from scratch.
- When the user finishes install and saves the profile, automatically select that profile in the existing profile list.
- Extend recent-file persistence to remember installer media paths in addition to game/trainer paths.
- Add a ranked `.exe` picker after install exit so common cases become one extra click instead of a full manual browse.

### Future Enhancements

- Optional `umu-run` integration or fallback when installed, for users who want a more Steam-runtime-like execution path.
- Advanced install arguments for installers that require silent switches, bootstrap flags, or launch options.
- Prefix templates or one-click post-install actions such as common dependencies, `winetricks`, or graphics/runtime presets.
- Import helpers for existing prefixes so users can point CrossHook at a game they installed earlier outside the app.
- Richer executable classification after install, such as filtering uninstallers, patchers, launchers, and crash reporters more aggressively.
- Install reports or profile notes that record which installer media, Proton version, and prefix were used originally.

### Risk Mitigations

- Installer exits before the real install flow is finished.
  - Mitigation: treat process exit as "install step completed" rather than "profile ready"; require explicit executable confirmation afterward.
- Prefix reuse leads to confusing or destructive behavior.
  - Mitigation: detect non-empty prefixes and show a confirmation warning before launch.
- Direct `proton run` is insufficient for some edge-case installers.
  - Mitigation: keep the install runner isolated behind its own command surface so optional `umu-run` support can be added later without redesigning the UI.
- Users save a broken profile because required runtime fields are incomplete.
  - Mitigation: validate Proton path, prefix path, installer path, and final game executable independently before final save.
- Feature scope balloons into a full Wine management tool.
  - Mitigation: keep v1 focused on "install one Windows game and generate one runnable profile" and defer advanced environment management.
