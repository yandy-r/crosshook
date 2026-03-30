# Custom Env Vars: Security Assessment

## Severity Summary

- **Critical**: Dangerous runtime key overrides can subvert launch behavior.
- **Warning**: Secret values may leak via logs/UI/export if not redacted.
- **Warning**: Oversized env payloads can fail process spawning.
- **Advisory**: Shell injection is mostly mitigated by Rust `Command` usage, but script paths still need careful handling.

## Findings

### CRITICAL: High-risk key overrides

User-defined env vars can interfere with runtime invariants if keys like `WINEPREFIX` or `STEAM_COMPAT_*` are overridden.

Mitigation:

- Blocklist runtime-critical keys in backend validation.
- Return clear validation errors before launch.

### WARNING: Secret exposure

Profile env vars may contain tokens/password-like values and can leak through:

- preview output
- logs/diagnostics
- profile export

Mitigation:

- Never log env values by default.
- If future UI supports masking, prefer masked rendering for sensitive-looking keys.
- Document that profile files are plaintext and should not store production secrets.

### WARNING: Payload size limits

Large total env data can trigger spawn failures (`E2BIG`) and poor UX.

Mitigation:

- cap number of entries
- cap key/value lengths
- fail fast with actionable error

### ADVISORY: Shell injection interpretation

CrossHook uses `Command` with explicit args/env and `env_clear()`, which avoids classic shell interpolation.

Residual risk:

- Shell helper scripts in launch paths must keep proper quoting.

Mitigation:

- Keep script-side handling quoted and avoid `eval`.

## Security Requirements for Issue #57

- Backend key/value validation must be authoritative.
- Runtime-critical keys cannot be overridden by custom env vars.
- Sensitive values are not emitted in routine logs.
- Preview and diagnostic output should avoid accidental secret disclosure.
