# Analysis Context: custom-env-vars

## Planning Synthesis

This feature is not a single-file change; it spans profile persistence, launch request validation, runtime command env application, preview output, and frontend profile editing. The highest-probability failure mode is semantic drift where different surfaces compute environment variables differently. To avoid that, implementation order should lock down shared merge behavior first, then fan out to runtime/preview/Steam outputs, then complete frontend authoring and validation UX.

## No-Deferral Completion Bar

A plan is acceptable only if it finishes with:

- profile schema + frontend typing fully wired,
- runtime launch behavior updated for all methods,
- preview and Steam launch options using identical merged env semantics,
- reserved-key and malformed input validation enforced in backend,
- automated tests validating persistence, precedence, security constraints, and parity.

## Non-Negotiable Design Rules

- One merge source of truth in core.
- Backend validation is final authority.
- Custom vars override optimization-derived vars.
- Reserved runtime keys remain protected.
- Feature is complete only when UX + runtime + preview + tests all pass.
