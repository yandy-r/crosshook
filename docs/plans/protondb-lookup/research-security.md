# Security Research: protondb-lookup

## Primary Risks

- ProtonDB responses are untrusted remote input, especially `launchOptions` and free-form notes.
- Direct browser fetch is blocked by ProtonDB CORS policy, which means backend ownership is mandatory and must not leak into ad hoc frontend workarounds.
- Applying raw launch strings would create a command-injection-like trust problem inside CrossHook’s own launch pipeline.

## Trust Boundaries

- Remote boundary: ProtonDB JSON comes from outside the app and must be validated, normalized, and size-bounded before storing or rendering.
- IPC boundary: only typed, Serde-safe DTOs should cross from `crosshook-core` into the frontend.
- Profile mutation boundary: user profiles are trusted local state; remote suggestions must not silently overwrite them.

## Required Safeguards

- Fetch in the backend only; never attempt to bypass CORS with browser hacks in the webview.
- Normalize exact tiers and recommendation structures into small internal DTOs before writing them into `external_cache_entries`.
- Render remote notes as plain text only.
- Parse launch option suggestions through a whitelist that accepts only supported `KEY=value` env-style fragments for apply actions.
- Keep unsupported/raw launch strings copy-only and never merge them directly into launch builders.
- Sequence overwrite handling before apply actions so existing `launch.custom_env_vars` remain user-owned.

## Failure-Handling Guardrails

- Remote outages, invalid payloads, or hidden report-feed failures must surface as soft advisory states.
- Cached data may be shown while stale, but the UI must mark it stale rather than pretending it is live.
- Empty Steam App IDs should be treated as “not configured,” not as a network or validation error.

## Open Risks

- The richer ProtonDB report feed discovered from the live site is undocumented and path-fragile; any dependency on it should be isolated behind summary-first fallback behavior.
- The existing `CompatibilityRating` naming suggests a false sense of parity with ProtonDB tiers even though it cannot represent `gold`, `silver`, `bronze`, or `borked` exactly.
