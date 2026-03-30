# Manual QA: custom environment variables (phase 4)

Operator checklist for [feature spec § Manual QA Matrix](../plans/custom-env-vars/feature-spec.md#manual-qa-matrix). Fill in **Result** and **Notes** when validating a release or PR; link this file or paste summaries in the related issue/PR.

**Build / version under test:** ********\_********  
**Tester:** ********\_******** **Date:** ********\_********

## Matrix

| #   | Scenario                                                                                                                                                                                             | Result (pass / fail / skip) | Notes |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------- | ----- |
| 1   | **`proton_run` — custom env only** — Add a custom var with no conflicting optimization; dry-run preview shows the var with source **Profile custom**; launch (or inspect command) applies it.        |                             |       |
| 2   | **`proton_run` — optimization only** — Enable an optimization that sets an env key; preview shows expected source (not profile custom).                                                              |                             |       |
| 3   | **`proton_run` — conflict** — Enable an optimization and set the **same key** under custom env with a different value; preview and runtime must show the **custom** value winning.                   |                             |       |
| 4   | **`steam_applaunch` — Steam launch options** — Generated line includes `KEY=value` tokens for custom vars; order matches spec (optimization env, then custom, then wrappers); `%command%` preserved. |                             |       |
| 5   | **`steam_applaunch` — conflict** — Same key in optimization + custom; copied line must encode the **custom** value for that key.                                                                     |                             |       |
| 6   | **`native` — custom env** — Custom vars appear in dry-run preview; smoke launch if practical.                                                                                                        |                             |       |
| 7   | **Persistence** — Save profile; confirm `launch.custom_env_vars` in `~/.config/crosshook/profiles/<name>.toml`; reload app and confirm UI round-trips.                                               |                             |       |
| 8   | **Validation / reserved keys** — Attempt `WINEPREFIX`, `STEAM_COMPAT_DATA_PATH`, or `STEAM_COMPAT_CLIENT_INSTALL_PATH` as custom keys; UI and launch validation should reject with clear messaging.  |                             |       |
| 9   | **Malformed keys** — Empty key, key containing `=`, NUL in key/value (if input allows): expect rejection.                                                                                            |                             |       |

## Acceptance cross-check

Maps to [feature-spec acceptance checklist](../plans/custom-env-vars/feature-spec.md#acceptance-checklist):

- [ ] Profiles support arbitrary key-value custom env vars (within validation rules).
- [ ] Custom vars apply at launch alongside optimization vars (`proton_run` / relevant paths).
- [ ] Custom vars take precedence on conflict.
- [ ] Preview shows merged effective env with custom source where applicable.
- [ ] Steam launch options generation reflects merged env.
- [ ] Runtime-critical reserved keys are blocked.

## Automated pre-check (CI / local)

Before manual QA, run:

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
cargo check --manifest-path src/crosshook-native/Cargo.toml --workspace
```
