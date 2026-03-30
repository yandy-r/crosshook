# Custom Env Vars: Engineering Practices Review

## Primary Practice Risk

The existing launch stack has separate environment-building logic in runtime and preview paths. Adding custom env vars independently to each path will create drift.

## Recommended Practice Decisions

### 1. One merge source of truth

Implement one `crosshook-core` helper that produces effective env entries from:

- method/runtime env
- optimization directives
- custom profile env

Use this helper in:

- runtime command creation
- launch preview
- steam launch-options rendering

### 2. Keep frontend thin

- Frontend should only edit/store `custom_env_vars` and send it in `LaunchRequest`.
- Validation and merge semantics remain in Rust backend.

### 3. Keep schema additive and simple

- `BTreeMap<String, String>` is sufficient for v1.
- Avoid introducing extra schema complexity (`unset` actions, priority flags) until needed.

### 4. Deterministic conflict behavior

- Last-write-wins in canonical merge helper.
- Custom env layer always applied last.

### 5. Test around shared helper

High ROI tests:

- precedence conflicts
- reserved key rejection
- method behavior parity
- preview/runtime consistency

## KISS Outcome

Feature-complete behavior with minimal risk comes from:

- additive model field
- single merge helper
- thin wiring across runtime/preview/frontend request builders
