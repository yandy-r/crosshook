# UX Research: protondb-lookup

## Executive Summary

The ProtonDB experience should feel like a contextual advisory card, not a second compatibility browser bolted into the profile form. Users already come to the profile editor to reconcile Steam metadata, trainer version, and launch settings, so ProtonDB guidance belongs beside those inputs with strong loading, stale-cache, and failure messaging. The UX should bias toward copy/apply assistance and transparent source freshness, while avoiding silent changes and noisy raw-report dumps.

### Core User Workflows

- Happy path:
  - User lands on a profile with a Steam App ID.
  - CrossHook shows a compact loading state in the same section as Steam metadata.
  - The panel resolves to an exact ProtonDB tier badge, freshness text, and a short recommendation stack.
  - The user can optionally copy raw launch options, apply supported env vars, or open the full ProtonDB page.
- Error recovery flow:
  - Lookup fails or times out.
  - The panel stays in place and explains that ProtonDB is unavailable while leaving the form fully usable.
  - If cached data exists, the UI keeps showing it with a stale badge instead of replacing it with an error wall.

### UI and Interaction Patterns

- Prefer a dedicated panel/card component rather than injecting additional status chips directly into existing field rows.
- Keep the exact tier badge visually distinct from CrossHook’s community compatibility badges, because the scale semantics are different.
- Group recommendations into:
  - supported/applyable env-var suggestions
  - copy-only launch string suggestions
  - plain-text notes or caveats
- Include a manual refresh action and a source link to the ProtonDB game page.
- If recommendations can overwrite existing profile env vars, surface a confirm-before-overwrite interaction instead of writing immediately.

### Accessibility Considerations

- Do not rely on badge color alone; every tier badge should include visible text.
- Loading, stale-cache, and unavailable states should be announced with standard text and not only iconography.
- Copy/apply buttons need explicit labels such as `Copy Launch Options` or `Apply Suggested Env Vars`.
- Notes and warnings should render as plain text blocks with good contrast and keyboard-focusable actions.

### Feedback and State Design

- Missing App ID: neutral empty state explaining that ProtonDB lookup starts once a Steam App ID is present.
- Loading: inline skeleton or muted loading copy inside the panel, not a full-form blocking spinner.
- Success: exact tier, report count, freshness time, and recommendation groups.
- Stale cache: keep the last good result visible with a stale label and refresh affordance.
- Unavailable: soft warning with retry, but no red validation treatment unless the user explicitly asked to refresh.

### UX Risks

- Users may read `working`-style CrossHook labels and ProtonDB exact tiers as equivalent; the UI must name ProtonDB explicitly.
- Raw community notes can be noisy or contradictory; CrossHook should summarize or group them rather than dumping a giant feed into the editor.
- Automatic application of launch strings would feel dangerous and opaque; the feature should default to explicit apply/copy actions.
- A panel that appears only for one launch method would feel inconsistent when a `proton_run` profile still has a valid Steam App ID, so the gating rule should be Steam App ID presence plus relevant Windows/Steam launch context.
