# Documentation Research: protonup-integration

## Architecture Docs

- `/AGENTS.md`: canonical architecture rules, persistence boundaries, IPC conventions, and verification requirements.
- `/.cursorrules`: same core repository constraints and implementation guardrails.

## API Docs

- `/docs/plans/protonup-integration/feature-spec.md`: primary API contract draft (`protonup_list_available_versions`, `protonup_install_version`) and phase breakdown.
- `/docs/plans/protonup-integration/research-external.md`: external API/service references (protonup-rs, GitHub releases).

## Development Guides

- `/docs/research/additional-features/implementation-guide.md`: feature implementation framing and process context.
- `/docs/plans/trainer-discovery/parallel-plan.md`: strong example of expected `parallel-plan.md` structure and task formatting style.

## README Files

- `/README.md`: top-level project setup and usage context.
- `/src/crosshook-native/README.md` (if present): native app workspace and build context for Rust + frontend integration.

## Must-Read Documents

- `AGENTS.md`: You _must_ read this when working on architecture placement, IPC naming, persistence classification, and release/commit conventions.
- `docs/plans/protonup-integration/feature-spec.md`: You _must_ read this when implementing provider scope, accepted decisions, and phase order.
- `docs/plans/protonup-integration/research-external.md`: You _must_ read this when integrating provider APIs and integrity checks.
- `docs/plans/protonup-integration/research-ux.md`: You _must_ read this when implementing launch-time prompts, progress/recovery states, and accessibility messaging.
- `docs/plans/protonup-integration/research-technical.md`: You _must_ read this when wiring module boundaries and file-level impacts.

## Documentation Gaps

- No existing dedicated document for Proton runtime provider abstraction patterns in `crosshook-core`; implementation should either add internal docs or annotate module docs.
- No explicit current runbook for installer failure categories and user-visible recovery messaging.
- No explicit documented naming/matching strategy for community `proton_version` normalization against installed runtime names.
