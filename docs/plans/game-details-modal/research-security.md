# Security Research: game-details-modal

## Executive Summary

The game details modal is primarily a frontend composition change, but it consumes remote-sourced metadata (Steam/ProtonDB) through existing IPC and cache layers. The main risks are unsafe rendering of remote text, introducing permissive new IPC inputs (paths/URLs), and weakening CSP or external-open constraints while adding convenience links/media. Existing guardrails in `crosshook-core` and Tauri capabilities should be reused and kept strict.

## Trust Boundaries

- WebView UI (`src/crosshook-native/src/`) renders remote/cache-backed strings and must treat them as untrusted display data.
- Tauri command boundary (`src-tauri/src/commands/*.rs`) is the validation chokepoint for any new data access.
- Core HTTP/cache modules (`crosshook-core/src/steam_metadata`, `protondb`, `game_images`) mediate remote requests and caching.
- Filesystem/SQLite layers persist cached artifacts and should be treated as potentially stale/user-influenced on read.

## Severity-Leveled Findings

### CRITICAL

- Remote metadata must not be rendered with raw HTML (`dangerouslySetInnerHTML`) in modal content; keep text rendering escaped/safe by default.
- Avoid adding generic fetch/read IPC endpoints that accept arbitrary URL/path inputs; keep fixed-host, normalized-input patterns.

### WARNING

- Do not broaden `img-src` policy to unrestricted remote hosts just to display headers; prefer existing `fetch_game_cover_art` local asset flow.
- If adding external links (for example, Steam store page), update `shell:allow-open` with narrowly scoped allowlists only.
- Any new import-from-path affordance should keep user-mediated file picker flow and existing image validation logic.

### ADVISORY

- Reuse existing HTTP timeout/user-agent and redirect controls for any new network helper.
- Keep cached payload parsing strict; avoid silently permissive deserialization for critical modal fields.
- Prevent modal UI from imitating privileged action prompts; keep dialog semantics and clear origin labeling.

## Required Guardrails

- Preserve existing input normalization patterns for app IDs and profile identifiers.
- Reuse command/hook contracts before adding new IPC surface.
- Render remote text as plain text or explicitly sanitized content.
- Keep modal close/focus semantics consistent with existing `crosshook-modal` patterns.
- Maintain strict capability and CSP boundaries when adding links or media.
- Add manual verification steps for offline/unavailable states to avoid misleading trust cues.

## Security Patterns to Reuse

- `src/crosshook-native/src-tauri/src/commands/game_metadata.rs`: validated command entry points for metadata/art.
- `src/crosshook-native/crates/crosshook-core/src/steam_metadata/models.rs`: app-id normalization and typed metadata parsing.
- `src/crosshook-native/crates/crosshook-core/src/game_images/client.rs`: redirect/image-byte validation safeguards.
- `src/crosshook-native/src/components/ProfileReviewModal.tsx`: focus, dialog, and close semantics.
- `src/crosshook-native/src-tauri/capabilities/default.json` and `src/crosshook-native/src-tauri/tauri.conf.json`: external-open and CSP guardrails.
