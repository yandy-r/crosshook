# Custom Env Vars: Business Analysis

## Feature Scope

GitHub issue `#57` requests per-profile custom environment variables with precedence over optimization-derived variables.

Core user value:

- Unblock edge-case game/trainer compatibility without waiting for catalog updates.
- Match expectations set by comparable Linux launchers.
- Keep profile behavior portable and reproducible.

## Expanded Acceptance Criteria

- Profiles store arbitrary `key -> value` custom env vars.
- Custom env vars are applied at launch time.
- Custom env vars and optimization env vars are both active when relevant.
- On conflict, custom env value wins.
- Launch preview surfaces effective merged values.
- Save/load roundtrip is stable and backward compatible.

## User Stories

- As a power user, I can define per-profile env vars not covered by built-in optimization toggles.
- As a troubleshooter, I can override a built-in optimization var for a specific game/profile.
- As a Steam method user, I can see and copy merged Steam launch options reflecting my custom overrides.
- As a reviewer, I can confirm effective env in preview before launch.

## Product Decisions Required For "No Deferred Scope"

- Apply custom env vars to all launch methods (`proton_run`, `steam_applaunch`, `native`) for consistent mental model.
- Expose validation errors inline in profile editing and via backend validation at preview/launch boundaries.
- Keep `custom env vars` in the portable profile launch section, not in local-only overrides.

## Risks If Scope Is Partial

- Runtime/preview drift if custom env applies in one path but not the other.
- User confusion if custom env works for `proton_run` but not `steam_applaunch` output.
- Support burden if precedence or duplicate handling is undefined.

## Business Definition Of Done

- User can create/edit/delete custom env vars in profile UI.
- Profile can be saved/reloaded with no data loss.
- Launch and preview both reflect merged env with custom precedence.
- Conflict behavior is deterministic and documented.
- Tests cover persistence, merge precedence, and method behavior.
