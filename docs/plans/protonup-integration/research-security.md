# Security Research: protonup-integration

## Executive Summary

The main risk surface is executing external installer/provider workflows and writing downloaded artifacts into compatibility tool directories. Security posture should enforce strict path validation, checksum verification, and clear trust boundaries between advisory recommendations and launch-blocking validation. The highest-risk failures are path traversal/write-outside-root, command injection via unsafe CLI invocation, and accepting unverified downloads.

## Trust Boundaries

- **UI input -> IPC boundary**: user-selected provider/version/target path crosses from frontend to Tauri commands.
- **IPC -> core execution boundary**: Tauri command arguments are passed into core install orchestration and potentially external tool invocation.
- **Network boundary**: remote release metadata and archives are fetched from third-party sources.
- **Filesystem boundary**: installer writes into compatibility tools directories that influence runtime execution paths.

## Severity-Leveled Findings

### CRITICAL

- Unsanitized command argument construction for provider installation can become shell injection if implemented with shell strings rather than argument vectors.
  - Mitigation: only use structured process args, no shell interpolation, explicit allowlist for provider identifiers.
- Path traversal or bad destination resolution could write outside expected Steam compatibility roots.
  - Mitigation: canonicalize and enforce prefix checks against discovered allowed roots before extraction/write.
- Marking install success before checksum/integrity verification could activate compromised or corrupted artifacts.
  - Mitigation: checksum verification is a required success gate.

### WARNING

- Stale or poisoned remote metadata can produce incorrect recommendations/install candidates.
  - Mitigation: bounded cache TTL, source labeling, and explicit stale indicators in UI.
- Provider binary/library auto-discovery on `PATH` may execute unexpected binaries.
  - Mitigation: optional explicit path setting, provenance logging, and binary existence/owner checks where feasible.
- Insufficient error categorization may lead users to unsafe retry behavior.
  - Mitigation: map failures to actionable categories (network, permission, integrity, dependency).

### ADVISORY

- Community `proton_version` strings are untrusted user content and may be ambiguous.
  - Mitigation: normalized matching with “unknown mapping” fallback; never auto-install solely from free-form metadata.
- Install operation logs may include local filesystem paths.
  - Mitigation: avoid excessive path disclosure in exported logs and sanitize UI-facing messages.

## Required Guardrails

- Enforce strict destination root checks before any filesystem write.
- Require checksum validation prior to successful install completion state.
- Keep recommendation mode advisory by default; launch hard-block remains tied to invalid configured runtime path only.
- Use explicit argument arrays for all subprocess execution and enforce provider/version input validation.
- Treat remote metadata as untrusted; label stale/offline states and avoid silent fallback ambiguity.

## Security Patterns to Reuse

- `/src/crosshook-native/crates/crosshook-core/src/install/service.rs`: process orchestration and structured error handling pattern.
- `/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`: cache-live-stale fallback with explicit error-path behavior.
- `/src/crosshook-native/src-tauri/src/commands/protondb.rs`: async command boundary pattern (`inner().clone()` before await and safe string mapping).
- `/src/crosshook-native/crates/crosshook-core/src/steam/proton.rs`: runtime path normalization and discovery patterns to avoid ad-hoc path handling.
