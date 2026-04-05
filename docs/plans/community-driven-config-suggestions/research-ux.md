# UX Research: ML-Assisted Configuration Suggestions

**Feature**: ML-assisted configuration suggestions from ProtonDB data (Issue #77)
**Date**: 2026-04-04
**Author**: ux-researcher agent

---

## Executive Summary

This document covers UX patterns and best practices for surfacing ML-assisted ProtonDB configuration suggestions inside CrossHook's profile creation and editing flows. The core challenge is presenting community-derived launch options and environment variables as trustworthy, non-intrusive optional prompts — without overwhelming users who may be mid-flow or unfamiliar with Proton tuning.

Key findings:

- Suggestions must be optional and dismissible at every interaction point. Force-showing them erodes trust and creates fatigue.
- Source attribution ("based on 47 ProtonDB reports") is a first-class UX requirement, not a nice-to-have. Users in the Linux gaming community are skeptical of opaque recommendations.
- A collapsible inline panel during profile creation/editing is the right pattern — not a modal, not a toast.
- Confidence tiers (Platinum-derived vs. single Gold report) should be visually distinct but not alarming.
- Offline and stale-cache states must communicate clearly without blocking the profile workflow.
- Apply suggestions one-at-a-time with individual accept/reject controls; provide a "apply all" shortcut but never make it the only path.

---

## User Workflows

### Visibility Gate

The ProtonDB suggestion panel is only shown when **both** conditions hold:

- `launchMethod ∈ {steam_applaunch, proton_run}`
- `steam.app_id` is non-empty

For all other launch methods (native, custom, etc.) the panel is hidden entirely. When a user switches launch method away from a Proton variant while editing, the panel should collapse with a brief explanation: "ProtonDB suggestions are available for Steam/Proton launch methods only." Do not silently hide it — users who expected the panel need to understand why it's gone.

### Workflow 1 — First-time Setup (No Existing Env Vars)

Profile created → game selected (App ID set, Proton launch method) → ProtonDB panel loads → suggestions appear → user clicks "Apply Suggested Env Vars" → no conflicts → applied immediately → status message: "Applied: PROTON_USE_WINED3D, PROTON_FORCE_LARGE_ADDRESS_AWARE" → env vars appear in `launch.custom_env_vars` table.

### Workflow 2 — Returning User (Env Vars Already Set, Conflicts Exist)

User opens existing profile for editing → ProtonDB suggestions load (stale-while-revalidate) → user clicks "Apply" → `ProtonDbOverwriteConfirmation` dialog opens for each conflicting key, showing current value vs. suggested value with "Keep current" / "Use suggestion" per-key buttons → user resolves each conflict → partial apply completes → status message confirms which keys were updated and which were kept.

Key UX requirement: the per-key conflict dialog must make it visually clear that each choice is independent. Do not present it as an all-or-nothing modal. Users must be able to accept some conflicting suggestions and reject others in the same flow.

### Workflow 3 — Catalog-Matched Suggestion (Optimization Toggle)

A suggestion matches a known optimization in the catalog (e.g., WINED3D renderer) → the suggestion item renders as "Enable WINED3D Renderer" with a toggle button instead of "Apply env var" → user clicks the toggle → the optimization switches on in the profile via the optimization catalog path → **no entry is added to `launch.custom_env_vars`** (the catalog manages it separately) → toggle reflects enabled state.

The UI must clearly distinguish catalog-matched suggestions (toggle + human-readable name) from raw env var suggestions (apply button + `KEY=VALUE`). Users need to understand these have different destinations.

### Workflow 4 — Offline with Stale Cache

User opens profile → network unavailable → ProtonDB banner: "Showing cached ProtonDB guidance (from 8 hours ago)" → suggestions are still displayed from cache and are still actionable → "Refresh" button is visible but shows a disabled or error state when clicked offline → user can apply cached suggestions normally → profile saves without requiring network.

### Primary Flow: Suggestions During Profile Creation

When a user is creating a new profile and selects a game (via Steam App ID with a Proton launch method), the system fetches or serves cached ProtonDB data. The suggestion surface appears **after game selection and launch method confirmation** and **before the user saves**.

**Step-by-step flow**:

1. User selects game and sets launch method to `steam_applaunch` or `proton_run`.
2. System begins async fetch/cache lookup for ProtonDB aggregated data.
3. While loading: a skeleton or spinner appears in a collapsible "ProtonDB Suggestions" panel below the launch options section. The panel does not block form interaction.
4. When data arrives: panel shows a badge ("3 suggestions available"). User opens the panel voluntarily.
5. Inside the panel: each suggestion is shown with its variable name or optimization name, suggested value or toggle, a confidence badge, a report count, and action controls (see Action Types below).
6. User accepts or dismisses individual suggestions, or uses "Apply all" / "Dismiss all" bulk actions.
7. Accepted env var suggestions populate `launch.custom_env_vars`; accepted catalog suggestions toggle the relevant optimization.
8. User continues to save the profile — no interruption to the save flow.

### Editing Flow: Suggestions on Existing Profiles

Show a subtle "New suggestions available" badge on the Suggestions panel when data has refreshed. Do not auto-expand or auto-apply. The badge is dismissible. Do not show a diff of what changed in ProtonDB — surface current aggregated suggestions as-is.

### On-Demand Flow: Manual Suggestion Refresh

A "Refresh" button in the panel header triggers a fresh lookup. Shows a loading state, updates cache, displays "Last checked: X minutes ago" on completion. This is also the retry path when the initial fetch failed.

---

## UI/UX Best Practices

### Suggestion Presentation

**Pattern**: Collapsible inline panel within the profile form, not a modal or floating overlay.

Rationale: Modals block workflow and feel intrusive for optional information. Floating overlays (toasts) are too transient for actionable suggestions. An inline collapsible panel is co-located with the form fields being populated, which is the correct mental model — users see where accepted suggestions will land.

**Panel anatomy**:

- Panel header: "ProtonDB Suggestions" with report count badge, confidence tier indicator, last-fetched timestamp, and a collapse/expand control.
- Panel body: a list of suggestion items, each occupying one row.
- Panel footer (when items exist): "Apply all" and "Dismiss all" secondary actions.

**Suggestion item anatomy** differs by suggestion type — three distinct action types exist:

**Type 1 — Raw env var suggestion** (most common):

- Left: `KEY` (monospace code chip) `=` `VALUE` (monospace code chip).
- Right: Source attribution chip (e.g., "47 reports"), confidence badge, and two action buttons: "Apply" (merges into `launch.custom_env_vars`) and "Copy" (copies `KEY=VALUE` to clipboard, always available as a fallback).
- Expandable: top 2–3 freeform report excerpts and Proton version range.

**Type 2 — Catalog-matched optimization** (when env var maps to a known optimization):

- Left: Human-readable optimization name (e.g., "Enable WINED3D Renderer").
- Right: Source attribution chip, confidence badge, and a toggle button ("Enable") that activates the optimization via the catalog path — does NOT add to `custom_env_vars`.
- Visual treatment: slightly distinct background or icon to signal this is a named feature, not a raw var.

**Type 3 — Launch argument suggestion**:

- Left: Raw launch argument string (monospace).
- Right: Source attribution chip, confidence badge, "Apply" button (appends to launch args field), and "Copy" button.

The "Copy" action must always be available on all types as a low-commitment fallback — users who don't trust the apply path can still get the value and handle it manually.

**Never** auto-apply suggestions without explicit user action, even when confidence is high.

### Confidence Indicators

Map confidence to a three-tier visual system derived from ProtonDB's own tier vocabulary:

| Tier   | Visual                                   | Threshold                                                    |
| ------ | ---------------------------------------- | ------------------------------------------------------------ |
| High   | Green badge, "Strong signal" label       | 10+ reports with this value, Platinum/Gold-weighted majority |
| Medium | Yellow/amber badge, "Some reports" label | 3–9 reports, mixed tier distribution                         |
| Low    | Gray badge, "Few reports" label          | 1–2 reports, or primarily Bronze/Unknown reporters           |

Avoid red for low confidence — red implies danger or error. Gray communicates "less certain" without alarming the user.

Each badge should have a tooltip explaining the threshold in plain language: "10 or more ProtonDB reports recommend this value."

### Source Attribution

Every suggestion must carry a visible report count. Format: "Based on N ProtonDB reports".

When N is small (1–2), add a qualifier: "Based on 2 ProtonDB reports — consider with caution."

Link the attribution chip to the ProtonDB game page (opens in system browser, not in-app WebView). This satisfies advanced users who want to read the raw reports themselves, and it is consistent with the existing ProtonDB lookup feature in CrossHook.

Do not attribute to specific usernames from ProtonDB reports — this is a privacy consideration and unnecessary for the suggestion value.

### Dismissal Patterns

**Individual dismiss**: each suggestion item has an X / Dismiss button. Dismissed suggestions do not reappear in the current session unless the user explicitly refreshes. Dismissal should be reversible — a brief "Undo" affordance (inline, fades after ~5 seconds) allows the user to restore a just-dismissed item. For longer-term recovery, dismissed suggestions should be accessible from a "Dismissed suggestions" section in profile settings (collapsed by default).

**"Dismiss all" / "Not now"**: collapses the panel and clears the badge. Suggestion data remains in cache; the panel is re-openable. This is a session-level action, not persistent.

**"Don't show for this game"**: a persistent per-profile preference accessible from a "..." overflow menu on the panel header. Suppresses the suggestion panel on subsequent edits of that profile. Must be reversible from profile settings.

**Persistence**: store dismissed-at timestamp and dismissed-suggestion fingerprints in SQLite per game profile (not globally). A user creating a second profile for the same game starts with a fresh suggestion state.

### Non-Intrusive Presentation

The suggestion panel must not:

- Interrupt the form tab/focus order (use `tabindex="-1"` on panel items when collapsed).
- Show more than 5–6 suggestions at once — paginate or truncate with "Show N more" if the extraction produces more.
- Auto-open on every edit — only on profile creation or when the "New suggestions" badge is explicitly clicked.
- Animate in a way that shifts form layout (use `position: absolute` or CSS reserved space to prevent layout shift).

---

## Error Handling

### Error States Table

| State                                      | User-visible message                                                                                                  | Panel behavior                                          | User action                                                           |
| ------------------------------------------ | --------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- | --------------------------------------------------------------------- |
| ProtonDB unreachable (network error)       | "Suggestions unavailable — couldn't reach ProtonDB."                                                                  | Panel shows error state with retry button               | Retry or continue without suggestions                                 |
| ProtonDB unreachable, stale cache exists   | "Showing suggestions from cached data (updated [date])."                                                              | Panel shows cached suggestions with staleness indicator | Accept cached suggestions or retry                                    |
| No App ID set (no game selected)           | Panel not shown                                                                                                       | Hidden                                                  | Select game first                                                     |
| No suggestions found for this game         | "No ProtonDB suggestions found for this game."                                                                        | Panel shows empty state with link to ProtonDB           | Continue without suggestions                                          |
| Suggestion conflicts with existing env var | `ProtonDbOverwriteConfirmation` dialog (already built) opens per conflicting key, showing current vs. suggested value | Per-key choice: "Keep current" or "Use suggestion"      | Resolve each conflict individually; unaffected keys apply immediately |
| Extraction produced no structured data     | "Couldn't extract settings from reports — data may be too sparse."                                                    | Panel shows message with link to ProtonDB               | Continue without suggestions                                          |
| Cache expired, refresh in progress         | "Refreshing suggestions..." with spinner                                                                              | Panel shows skeleton over cached data                   | Wait or cancel                                                        |

### Offline Behavior and Staleness Indicators

Three precise freshness states drive the UI, mapped directly from `ProtonDbCacheState`:

| Cache state                           | UI treatment                                                                                                                                                                                                 |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `from_cache = true, is_stale = false` | Subtle gray label in panel header: "Loaded from local cache · Updated X hours ago". No banner.                                                                                                               |
| `is_stale = true`                     | Amber warning banner above suggestions: "Showing cached ProtonDB guidance because the live lookup failed." Cache age shown as relative time (e.g., "from 8 hours ago"). Suggestions remain fully actionable. |
| `unavailable` (no cache, no network)  | Neutral/warning banner: "ProtonDB is unavailable." Panel shows no suggestions. Retry button present.                                                                                                         |

In all states, profile creation and saving remain fully unblocked. Suggestions are supplemental.

When offline with a stale cache (`is_stale = true`), the "Refresh" button is visible but should show an error state when clicked ("Can't refresh — no network connection") rather than silently doing nothing.

### Security Note on Error Messages

Error messages must not expose:

- Raw API URLs or endpoint paths.
- HTTP status codes (e.g., "503 from api.protondb.com").
- Internal cache file paths.

Error messages should be plain-language and user-facing only. Backend errors are logged internally; the UI shows a simplified message. This prevents information leakage about CrossHook's internal architecture.

---

## Performance UX

### Loading States While Fetching ProtonDB Data

The suggestion panel must never block the profile creation form from being usable. Loading is always asynchronous.

**Loading state design**:

- Collapsed panel with a spinner inside the header: "Fetching suggestions..."
- Or a skeleton loader showing 2–3 placeholder suggestion rows (gray bars at correct heights).
- Prefer skeleton over spinner for layouts where the panel is already partially expanded.

Do not show a full-page or modal loading overlay. This is supplemental data, not required for the form to function.

**Timeout handling**: If the ProtonDB fetch takes more than 6 seconds, surface the panel in an error/retry state rather than continuing to spin indefinitely. This matches the existing 6-second timeout already configured in `useProtonDbLookup.ts` — do not introduce a conflicting value. Users should not be waiting for optional data.

### Progressive Disclosure of Suggestions

When suggestions arrive, do not immediately expand and render all items simultaneously. Recommended approach:

1. Panel badge updates to show "N suggestions available."
2. User opens panel manually (or it opens automatically on first use with a subtle animation).
3. Top 3 highest-confidence suggestions are visible by default.
4. Remaining suggestions are behind a "Show N more suggestions" expandable.
5. Clicking individual suggestion rows reveals the supporting evidence (report excerpts).

This mirrors the ProtonDB UX itself — the game summary tier is shown first, with individual user reports discoverable on demand.

### Optimistic Caching Behavior

Once ProtonDB data is fetched for a game, cache it in the local SQLite metadata DB with:

- App ID as key
- Suggestion payload as JSON blob
- Fetched-at timestamp
- TTL: 24 hours (configurable)

On subsequent profile creation/editing for the same game, serve from cache immediately while a background refresh happens (stale-while-revalidate). The user sees suggestions instantly; if new data arrives, update the panel in-place with a subtle "Updated" indicator.

This means suggestion latency is near-zero for repeat visits to the same game's profile.

### Background Prefetch (Nice-to-Have)

When the user opens the game library, the system could speculatively prefetch ProtonDB data for the top N recently-played or frequently-profiled games. This would make suggestion loading instant when those games are opened for profile editing. Prefetch should be rate-limited, respect user privacy settings, and run only when idle (no active profile edit in progress).

---

## Competitive Analysis

### ProtonDB Itself

ProtonDB presents per-game compatibility as a tiered badge (Platinum > Gold > Silver > Bronze > Borked) derived from crowdsourced reports. Individual reports are shown in chronological reverse order, each containing: rating, Proton version, OS/distro, GPU, freeform "notes" text, and a "helpful" vote.

**What works**: The tier badge gives an instant at-a-glance signal. Report filtering by hardware/distro adds personalization. The notes section is unstructured but powerful — it's where launch options and workarounds live.

**What doesn't work**: There is no aggregation of the freeform notes. If 40 users all mention `DXVK_ASYNC=1` in their notes, there's no roll-up — users must read through reports individually. CrossHook's ML extraction is designed to solve exactly this gap.

**Pattern to adopt**: The tiered confidence model (Platinum-equivalent = high, Gold = medium, Silver/Bronze = low) maps naturally to CrossHook's three-tier confidence display. The "helpful" vote pattern is worth considering for future feedback collection on suggestions.

**Confidence**: High — first-hand knowledge of the product, confirmed by multiple user and documentation sources.

### Lutris

Lutris automates game installation via community scripts that bundle Wine configuration, environment variables, and launch arguments. Configuration is set through the installation script itself — users don't see individual suggestions, they get the whole tuned configuration as a package.

**What works**: Zero friction — if the script is good, the game just works. Scripts are community-maintained with version history.

**What doesn't work**: It's all-or-nothing. There's no individual suggestion model. Users can't selectively apply parts of a script's configuration. The UX for editing launcher settings post-install (Game options > System options > Advanced) is tab-heavy and requires knowing what fields exist.

**Pattern to avoid**: Don't replicate the all-or-nothing installation script model. CrossHook should give users granular control over which suggestions to accept.

**Pattern to adopt**: The separation of global defaults vs. per-game overrides (Lutris has both system-level and game-level env var tables) maps to CrossHook's profile-level settings. The "Advanced" tab pattern for surfacing env var management is familiar to the Linux gaming audience.

**Confidence**: High — confirmed from Lutris GitHub issues and community forum posts.

### Bottles

Bottles presents configuration through a "bottle" (Wine prefix) model. Each bottle has a settings panel with toggles for DXVK, VKD3D, Esync/Fsync, and environment variable tables. Configuration is per-bottle, not per-game.

**What works**: Toggle-based presentation of common compatibility flags (DXVK on/off) is more approachable than raw env var strings. GNOME Adwaita UI patterns are clean and predictable.

**What doesn't work**: Bottles does not aggregate community data or suggest configurations. Suggestions would require users to manually look up ProtonDB and then translate that into Bottles settings.

**Pattern to adopt**: Toggle-based presentation for common flags (DXVK, Esync, Fsync) alongside the raw env var panel. CrossHook's suggestion UI could offer two views: "Simple" (named toggles for known common flags) and "Advanced" (raw env var name/value). Accept a suggestion for `DXVK_ASYNC=1` → auto-populate the Advanced view; also reflect it as an enabled toggle in the Simple view.

**Confidence**: Medium — based on community comparisons and Bottles documentation; no direct API/source review.

### Steam Deck Verified Badge System

Valve's compatibility program uses four tiers (Verified, Playable, Unsupported, Unknown) surfaced as badge icons in the store and library. Users can filter/sort their library by compatibility tier.

**What works**: The badge system is instantly scannable. Integration at the library level means users encounter compatibility signals during browsing, not just during configuration. The Playable tier communicates "works with effort" — a nuanced middle ground.

**What doesn't work**: The badges are binary in nature (a game either is or isn't Verified). They don't surface _why_ or _how_ to fix Playable issues. Community feedback (ProtonDB Badges plugin for Steam Deck) supplements this gap. The Verified badge does not account for performance, only compatibility — a known credibility issue.

**Pattern to adopt**: The library-level integration pattern (showing a compatibility badge next to game names) is valuable context. CrossHook could show a ProtonDB tier indicator next to game names in its library view, providing ambient awareness before users open the profile editor. This reduces context-switching.

**Pattern to avoid**: Don't make CrossHook's confidence system feel like a compliance gate. Steam Deck Verified is often inaccurate; community-sourced confidence is inherently probabilistic and should be presented as guidance, not certification.

**Confidence**: High — confirmed by Steamworks documentation and multiple third-party analyses.

### Heroic Games Launcher

Heroic provides per-game environment variable and wrapper command tables in a dedicated "Advanced" settings tab. Version 2.16.0 added improved env var UX including table-based editing with key/value fields and a + button to add rows.

**What works**: The table model for env vars (key column, value column, add/remove row buttons) is familiar to developers and power users. Separating global settings from per-game settings is correct. The Known Fixes repository (`Heroic-Games-Launcher/known-fixes`) is an attempt at curated community knowledge, though not ML-aggregated.

**What doesn't work**: Heroic does not aggregate ProtonDB data or surface suggestions automatically. Users must know what to enter. The Known Fixes repo requires manual lookup and copying.

**Pattern to adopt**: The key/value table UI for env vars should be CrossHook's existing pattern for the Advanced settings section. Accepted suggestions should populate this same table rather than introducing a separate data model.

**Confidence**: High — confirmed from Heroic changelog, GitHub issues, and community documentation.

### Other Recommendation UIs (Non-Gaming)

**GitHub Copilot** (VS Code inline suggestions): Ghost text suggestions appear inline as the user types, accepting with Tab. Source attribution is surfaced via "Code Referencing" when matches are found in public repos. The key UX insight: suggestions appear in context (at the cursor), not in a separate panel. For CrossHook, the analogue is surfacing suggestions adjacent to the fields they affect.

**IDE Package Vulnerability Advisories** (e.g., npm audit in VS Code extension): These surface inline warnings on specific `package.json` lines with a "Why?" expandable and a "Fix" action button. The pattern — inline, co-located with the affected item, with an explanation path — is directly applicable to CrossHook's per-suggestion expand-for-details behavior.

**Confidence**: Medium — GitHub Copilot UX is well-documented; package advisory pattern is observed practice.

---

## Recommendations

### Must Have

1. **Collapsible inline suggestion panel** shown only when `launchMethod ∈ {steam_applaunch, proton_run}` AND `steam.app_id` is set. Explain why the panel is hidden when users switch launch methods. Never block the form. Never auto-apply.

2. **Three distinct action types** with clear visual differentiation: "Apply" for raw env var suggestions (→ `launch.custom_env_vars`), "Enable [Name]" toggle for catalog-matched optimizations (→ optimization catalog, not `custom_env_vars`), and "Copy" always available as a low-commitment fallback on all types. Per-suggestion dismiss controls with brief undo affordance.

3. **Source attribution on every suggestion**: "Based on N ProtonDB reports" with a link to the ProtonDB game page. Report count is a trust signal; omitting it removes credibility.

4. **Three-tier confidence display** using color-coded badges (High/Medium/Low). Tooltips explain the threshold in plain language. Do not use red for low confidence.

5. **Stale-cache offline behavior**: Show cached suggestions when ProtonDB is unreachable, with a clear "Cached — [timestamp]" label. Never block profile creation due to network unavailability.

6. **Error states for all failure modes** (network error, no data, extraction failure) that communicate clearly in plain language without exposing internal technical details.

7. **Per-game dismissal persistence** in SQLite: dismissed-at timestamp and dismissed suggestion fingerprints, scoped to the profile (not globally).

8. **"Last checked" timestamp** visible in the panel header. Users in the Linux gaming community expect to know how fresh community data is.

### Should Have

9. **Expandable suggestion details**: clicking a suggestion row reveals the top 2–3 freeform report excerpts that contributed to it, plus the Proton version range where it was most common.

10. **Stale-while-revalidate caching**: serve cache immediately on panel open, refresh in background, update panel in-place with a "Updated" indicator.

11. **Suggestion count badge** on the Suggestions panel header that is visible even when collapsed, so users know suggestions are available without opening the panel.

12. **Conflict resolution via existing `ProtonDbOverwriteConfirmation` dialog**: per conflicting key, show current vs. suggested value with independent "Keep current" / "Use suggestion" buttons. The dialog is already built — do not replace it with a different pattern. Ensure it is invoked per-key (not per-batch) so users can make independent choices.

13. **Timeout handling**: if ProtonDB fetch takes > 5 seconds, show error/retry state rather than indefinite spinner.

14. **Simple/Advanced toggle** for well-known flags: common flags like DXVK, Esync, Fsync surfaced as named toggles alongside the raw env var table. Accepting a suggestion auto-populates both views consistently.

### Nice to Have

15. **"Don't show for this game" option** in a panel overflow menu for users who have their own tuning preferences.

16. **Background prefetch** for recently-played games when the user opens the library (idle, rate-limited).

17. **ProtonDB tier badge** in the library game list view, providing ambient compatibility awareness before the user opens a profile.

18. **User feedback on suggestions**: a "Was this helpful?" thumbs up/down on each accepted suggestion after a subsequent launch, to improve future extraction quality.

---

## Open Questions

1. **Auto-open on first use**: Should the suggestion panel auto-expand the first time a user creates a profile for a game, to build discoverability? Or should it always require a manual open? This trades discoverability against intrusiveness.

2. **Minimum report threshold**: What is the minimum number of ProtonDB reports before the system surfaces any suggestion at all? Surfacing suggestions from a single report risks misleading users. A floor of 3 reports is a reasonable default; this should be a configurable backend parameter.

3. **Extraction accuracy display**: If the ML extraction has an internal accuracy metric for a specific suggestion, should this be surfaced to the user beyond the three-tier confidence band? Exposing raw percentages may confuse non-technical users; hiding them may frustrate power users who want full transparency.

4. **Suggestion versioning**: ProtonDB data changes as new reports come in. Should CrossHook version its suggestions (e.g., "suggestion v1 accepted on date X") so users can see if the community consensus has shifted since they last accepted?

5. **Integration with conflict resolution**: The `ProtonDbOverwriteConfirmation` dialog handles cases where current ≠ suggested. For cases where current = suggested (values already match), the UX should silently skip — surfacing a confirmation for a non-conflict creates noise without value. This is a recommendation, not yet confirmed as the implemented behavior.

6. **Scope of suggestions**: Should suggestions cover only environment variables and launch arguments, or also Proton version recommendations? ProtonDB reports often specify which Proton version worked — surfacing this as a suggestion adds significant value but also complexity.

---

## Sources

- [ProtonDB — Gaming know-how from the Linux and Steam Deck community](https://www.protondb.com/)
- [ProtonDB | Help | Improving Performance](https://www.protondb.com/help/improving-performance)
- [GitHub — bdefore/protondb-data: Data exports from ProtonDB.com under ODbL](https://github.com/bdefore/protondb-data)
- [GitHub — Trsnaqe/protondb-community-api](https://github.com/Trsnaqe/protondb-community-api)
- [Heroic Launcher 2.16.0 Update — Steam Deck HQ](https://steamdeckhq.com/news/heroic-launcher-2-16-0-update/)
- [Heroic Games Launcher — Environment Variables Wiki](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/wiki/Environment-Variables)
- [Settings Interface — Heroic Games Launcher DeepWiki](https://deepwiki.com/Heroic-Games-Launcher/HeroicGamesLauncher/4.4-settings-interface)
- [UI for launch_configs — Lutris GitHub Issue #4648](https://github.com/lutris/lutris/issues/4648)
- [Steam Deck Compatibility Review Process — Steamworks Documentation](https://partner.steamgames.com/doc/steamdeck/compat)
- [Steam Deck Compatibility: Understanding Verified, Playable, and Unsupported](https://www.steamnavigator.com/blog/steam-deck-compatibility)
- [Why You Can't Trust Steam Deck Verified Labels](https://www.howtogeek.com/why-you-cant-trust-steam-deck-verified-labels-and-what-to-do-about-it/)
- [ProtonDB Badges Steam Deck Plugin](https://steamdecklife.com/2022/10/18/protondb-badges-steam-deck-plugin/)
- [UX Design for AI Products — Tenet](https://www.wearetenet.com/blog/ux-design-for-ai-products)
- [Confidence Visualization UI Patterns — Agentic Design](https://agentic-design.ai/patterns/ui-ux-patterns/confidence-visualization-patterns)
- [Best Practices for Notifications UI Design — Setproduct](https://www.setproduct.com/blog/notifications-ui-design)
- [Notifications — Carbon Design System](https://carbondesignsystem.com/patterns/notification-pattern/)
- [Design Guidelines for Better Notifications UX — Smashing Magazine](https://smart-interface-design-patterns.com/articles/notifications/)
- [Progressive Disclosure in UX Design — LogRocket](https://blog.logrocket.com/ux-design/progressive-disclosure-ux-types-use-cases/)
- [Progressive JSON — Dan Abramov / overreacted](https://overreacted.io/progressive-json/)
- [AI UX Patterns for Design Systems — The Design System Guide](<https://thedesignsystem.guide/blog/ai-ux-patterns-for-design-systems-(part-1)>)
- [Designing for Trust: UI Patterns That Build Credibility](https://medium.com/@Alekseidesign/designing-for-trust-ui-patterns-that-build-credibility-e668e71e8d47)
- [10 UX Design Patterns That Improve AI Accuracy and Customer Trust — CMSWire](https://www.cmswire.com/digital-experience/10-ux-design-patterns-that-improve-ai-accuracy-and-customer-trust/)
- [Inline Validation UX — Smart Interface Design Patterns](https://smart-interface-design-patterns.com/articles/inline-validation-ux/)
- [Offline-First Frontend Apps in 2025 — LogRocket](https://blog.logrocket.com/offline-first-frontend-apps-2025-indexeddb-sqlite/)
- [GitHub Copilot code suggestions — GitHub Docs](https://docs.github.com/en/copilot/concepts/completions/code-suggestions)
- [Accept and reject individual suggestions — Waze User Voice (reference UX pain point)](https://waze.uservoice.com/forums/59225-map-editor-suggestions/suggestions/46169668-accept-and-reject-individual-suggestions-in-pur)
