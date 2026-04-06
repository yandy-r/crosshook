# UX Research: Trainer Discovery Feature

## Executive Summary

Trainer discovery requires a search interface that bridges the gap between installed games and available trainer sources. The primary UX challenge is guiding users from "I want cheats for this game" to "I have a compatible trainer configured in my profile" — across a domain where version mismatches, untrusted sources, and offline constraints are common failure modes.

The existing CrossHook UI already establishes strong patterns: the `ProtonDbLookupCard` demonstrates tiered compatibility badges, cached-data banners with stale states, external source linking via Tauri's `open`, and progressive disclosure of community recommendations. The `LibraryCard` shows skeleton loading on lazy-loaded images. The `CommunityBrowser` demonstrates live search filtering with `matchesQuery` against composite haystack strings. Trainer discovery should extend these patterns rather than introduce new paradigms.

**Confidence**: High — based on direct codebase analysis plus competitive analysis of WeMod, Nexus Mods, ProtonDB, FLiNG Trainer, and PCGamingWiki.

---

## User Workflows

### Primary Flow: Game-First Discovery (Happy Path)

The user starts from a known game and wants to find compatible trainer sources.

```
1. Open "Discover Trainers" panel
2. SQLite tap results load immediately — no spinner for tap data
   └── Game pre-filled from active profile (game_name, steam_app_id) if available
   └── If no active profile: search field is empty, user types game name
3. System checks network status:
   └── Offline → persistent offline banner above results; tap-only results shown
   └── Online → tap results visible; "Search Online" button available
4. User types game name → real-time substring filter (same matchesQuery logic as CommunityBrowser)
   └── Filters across: game_name, trainer_name, description, platform_tags
   └── Results sorted: tap results first (compatibility_rating desc, then game_name), external results below
5. User expands a result card → sees detail section:
   └── trainer_name, trainer_version, game_version, proton_version, platform_tags
   └── Compatibility rating badge + version correlation status badge
   └── SHA-256 (if present): "Verified by community" label + collapsible raw hash
6. User action depends on result type:
   └── Has community profile: "Import Profile" (primary CTA) + "Get Trainer ↗" (secondary)
   └── Link only: "Get Trainer ↗" (sole CTA) — Tauri openUrl(), no in-app download
7. User downloads trainer externally, configures path in profile TrainerSection
```

**Decision points that map to business rules:**

- Tap results vs. external results → visual trust tier (see Trust Indicators section)
- Version correlation state (Untracked / Matched / GameUpdated / TrainerChanged / BothChanged) → badge
- Has community profile → dual CTA; link-only → single CTA
- Offline → show tap-only results with persistent banner; never show blank screen

### Alternative Flow: Search-First Discovery

The user does not have an active profile and searches by game name.

```
1. Panel opens with empty search field, SQLite tap results visible (all games)
2. User types → real-time filter narrows results
3. User clicks "Search Online" to fetch external results (explicit, not automatic)
4. User expands a result and proceeds from step 5 of primary flow
```

### Alternative Flow: Profile-Inline Discovery

The user is editing a profile and the TrainerSection surfaces a discovery shortcut.

```
1. User sees "Find trainers for this game ↗" button in TrainerSection
2. Opens discovery panel pre-filtered to the profile's game
3. User imports a profile or copies a source link and returns to profile editor
```

---

## UI/UX Best Practices

### Search Interface

**Input field:**

- Single search input at the top of the panel, keyboard-focusable on panel open
- Placeholder: "Search games or trainers…"
- 300ms debounce — the industry consensus for search inputs that trigger API calls
- Clear button (×) appears when input has content
- Instant local filtering on cached results while remote fetch is in flight

**Result count:**

- Display "N results" subtitle beneath the search field; update reactively
- When zero results: do not render empty results silently (see Error Handling section)

**External search:**

- Network calls are never implicit. An explicit "Search Online" button triggers external source queries.
- This mirrors the existing ProtonDB card "Refresh" button pattern — user controls when network is hit.
- When online, "Search Online" is enabled. When offline, it is disabled with tooltip "You're offline".

**Sort and filter controls:**

- Secondary toolbar with: compatibility filter (All / Working / Partial / Broken / Unknown), source type filter (Community Taps / All)
- Minimal by default — don't expose filters until results are visible

**Industry precedent (WeMod):** category browsing + option count per trainer upfront. Users want to know depth before committing to a download. Apply this as a "N cheat options" metadata tag in result cards.

### Result Cards

Each result card (collapsed) surfaces:

| Field                      | Prominence      | Notes                                                                |
| -------------------------- | --------------- | -------------------------------------------------------------------- |
| Game name                  | Primary         | Bold, large                                                          |
| Trainer name               | Secondary       | Below game name                                                      |
| Compatibility rating badge | Right-aligned   | ProtonDB-style tier (platinum/working/partial/broken/unknown)        |
| Version correlation badge  | Inline          | Matched/GameUpdated/TrainerChanged/BothChanged/Untracked (see below) |
| Trust indicator            | Icon + label    | "Community" badge or chain-link icon (see Trust Indicators)          |
| Source tap name            | Muted text      | Tap URL hostname or tap display name                                 |
| Last updated               | Muted timestamp | formatRelativeTime()                                                 |

Expanded detail section (on card click/enter):

| Field                                         | Notes                                                                         |
| --------------------------------------------- | ----------------------------------------------------------------------------- |
| trainer_version, game_version, proton_version | Raw version strings                                                           |
| platform_tags                                 | Displayed as tags                                                             |
| SHA-256                                       | "Verified by community" label; raw hash in collapsible via CollapsibleSection |
| Primary CTA                                   | "Import Profile" if community profile exists; else absent                     |
| Secondary CTA                                 | "Get Trainer ↗" — always present if source_url available                      |

**Progressive disclosure** via the `CollapsibleSection` component already in use. Collapsed cards are scannable; expanded cards give full detail including version strings and SHA-256.

### Compatibility Badges

Follow the ProtonDB tier badge pattern already established in `ProtonDbLookupCard`. The backend `checkVersionMatch()` call returns one of 5 statuses; the badge must handle all of them:

| Backend status    | Badge class modifier | Color token                             | Label              |
| ----------------- | -------------------- | --------------------------------------- | ------------------ |
| `exact`           | `--exact`            | `--crosshook-color-success` (#28c76f)   | "Exact match"      |
| `compatible`      | `--compatible`       | `--crosshook-color-success` (#28c76f)   | "Compatible"       |
| `newer_available` | `--newer-available`  | `--crosshook-color-warning` (#f5c542)   | "Update available" |
| `outdated`        | `--outdated`         | `--crosshook-color-danger` (#ff758f)    | "Outdated"         |
| `unknown`         | `--unknown`          | `--crosshook-offline-unknown` (#9e9e9e) | "Unknown"          |

**Two-stage render:** Version match is fetched on-demand per result via `checkVersionMatch()`, not pre-computed across the full results list. The badge must handle its own loading state before the status resolves — render a neutral gray placeholder within the badge area while the call is in-flight. Do not delay rendering the result card itself waiting for the version check.

Always pair color with text label — never rely on color alone (WCAG accessibility requirement).

**Confidence**: High — IBM Carbon Design System, Ant Design, and Dell Design System all converge on this traffic-light pattern for status indicators.

### Version Correlation Badges

Separate from the compatibility rating badge, the version correlation status reflects whether the trainer has been tested against the user's currently installed game build. The business layer returns 5 states; map to 3 visual tiers to avoid cognitive overload:

| Business state   | Visual tier | Color                         | Label             | Tooltip                                                                 |
| ---------------- | ----------- | ----------------------------- | ----------------- | ----------------------------------------------------------------------- |
| `Matched`        | Green       | `--crosshook-color-success`   | "Version matched" | "Trainer was tested on your installed version"                          |
| `GameUpdated`    | Yellow      | `--crosshook-color-warning`   | "Game updated"    | "Your game was patched since this trainer was tested — it may not work" |
| `TrainerChanged` | Yellow      | `--crosshook-color-warning`   | "Trainer updated" | "This trainer has been updated — re-verify if you have issues"          |
| `BothChanged`    | Yellow      | `--crosshook-color-warning`   | "Both updated"    | "Both the game and trainer have changed since last verification"        |
| `Untracked`      | Gray        | `--crosshook-offline-unknown` | "Not tracked"     | "This game has not been launched with CrossHook yet"                    |

Three yellow states share a visual tier but have distinct tooltip text — the tooltip is required to distinguish them. Do not create three separate colors for `GameUpdated`/`TrainerChanged`/`BothChanged`; three yellows convey the same actionable meaning ("proceed with caution") and more colors increase cognitive load.

**Two-stage render:** Version correlation is fetched on-demand via `checkVersionMatch()` after the card renders. Show a neutral gray placeholder badge ("Checking…") while in-flight.

### Source Trust Indicators

Two-tier model: tap results vs. external results. A three-tier model (subscribed tap / unsubscribed tap / unverified) is too nuanced for the result list — the meaningful distinction at glance is "did CrossHook's community curate this, or did an algorithm find it?"

| Source type                  | Visual                     | Label                   | Behavior                                               |
| ---------------------------- | -------------------------- | ----------------------- | ------------------------------------------------------ |
| Community tap result         | Filled badge, accent color | "Community"             | No confirmation on "Get Trainer ↗"                     |
| External / unverified result | Chain-link icon, muted     | _(no label, icon only)_ | Optional single confirmation dialog for non-https URLs |

Display a tooltip on hover explaining what each indicator means. Never block the user from opening a link based on trust level — trust indicators are informational only.

**Do not display security warnings as errors** for unverified sources — alert fatigue causes dismissal of all warnings including legitimate ones. Reserve modal-level warnings for non-https source URLs only.

### Accessibility

- All interactive elements meet `--crosshook-touch-target-min: 48px` (already defined in variables.css)
- Controller mode: respect `--crosshook-touch-target-min: 56px` defined in `[data-crosshook-controller-mode='true']`
- Keyboard navigation: Tab through search → filters → result cards; Enter to expand a card; Enter/Space on "Open Source" button
- Screen reader: ARIA live region on results count (`aria-live="polite"`) so count updates are announced
- Skeleton loaders: use `role="status"` and `aria-label="Loading trainer results"` on skeleton container; honor `prefers-reduced-motion` for shimmer animation
- Compatibility badges: include `aria-label` with full text, e.g., `aria-label="Compatible with installed version"`

---

## Error Handling

### Error States Table

| Scenario                        | Banner tone              | Message                                                                     | User action                         |
| ------------------------------- | ------------------------ | --------------------------------------------------------------------------- | ----------------------------------- |
| Offline, taps previously synced | Unavailable (persistent) | "You're offline. Showing local tap results only."                           | None blocking — results are visible |
| Offline, no taps ever synced    | Unavailable              | "No community taps configured. Add a tap to discover trainers."             | "Add Community Tap" CTA             |
| No results found (tap search)   | Neutral                  | "No trainers found for '[query]'. Try different terms or search online."    | "Search Online" button              |
| External search failed          | Stale                    | "Online search unavailable. Showing local results only."                    | "Retry" button                      |
| External search returns zero    | Neutral                  | "No online results found for '[query]'."                                    | Suggest refining query              |
| Tap sync stale                  | Stale                    | "Tap data last updated [relative time]. Refresh to check for new trainers." | "Refresh" button                    |
| Source URL absent               | Inline                   | Gray out "Get Trainer ↗"; tooltip: "Source URL not available"               | None                                |
| Version correlation pending     | Inline in badge          | "Checking…" gray placeholder badge                                          | Auto-resolves                       |

**Offline notice placement:** Persistent inline banner above the results list (not a modal, not a toast). The `OfflineStatusBadge` component is too subtle when missing network means missing an entire category of results. Use the existing `--unavailable` banner modifier tone.

**Banner component reuse:** The existing `crosshook-protondb-card__banner` pattern with `--neutral`, `--stale`, `--loading`, `--unavailable` modifiers maps directly. Use the same modifier names for trainer discovery banners.

### Validation Patterns

- Do not validate the search input beyond trimming whitespace — any non-empty string is a valid query
- Do not show inline validation errors on the search field; show the no-results state instead
- Version range parsing failures: show the raw version string rather than an error; log to console (Tauri side)

---

## Performance UX

### Search Debouncing

- Debounce delay: **300ms** (industry consensus for search inputs)
- Local cache filter fires immediately on each keystroke (synchronous, no debounce)
- Remote refresh (if online) debounced to **500ms** to avoid hammering community tap APIs
- Show local cache results immediately during the remote fetch — do not blank the list

### Skeleton Loading States

Apply the existing `crosshook-skeleton` class (defined via `--crosshook-skeleton-duration: 1.8s` and skeleton color variables) to placeholder cards:

```
Initial panel open (no cache):
  → 3-4 skeleton result cards visible
  → Count shows "Loading…"
  → Search field is enabled immediately
  → Skeleton clears when first results arrive
```

```
Cache hit (instant):
  → Results render immediately, no skeleton
  → "Background refresh" spinner (small inline indicator) in toolbar
  → Spinner clears when refresh completes
```

**Skeleton accessibility:** `@media (prefers-reduced-motion: reduce)` — replace shimmer animation with static placeholder color. The existing `--crosshook-skeleton-duration` variable already enables this via CSS.

### Cached Results Display

Mirror the `ProtonDbLookupCard` stale-cache pattern:

- Show results from cache immediately
- Render stale banner: "Showing cached trainer data from [relative time]"
- Replace banner with "Updated" confirmation for 2 seconds after background refresh completes, then remove it
- `formatRelativeTime()` utility already exists — reuse it

### Background Refresh Indicators

- Small spinner icon in the panel toolbar right side (not blocking the results)
- Label: "Refreshing…" only visible to screen readers via `aria-label`
- On completion: brief "Updated" text (2s), then disappear
- Never block the UI for background refreshes

### Optimistic Updates

When a user subscribes to a new community tap from within the discovery panel:

- Add the tap to the subscribed list immediately (optimistic)
- Show tap results from local cache if available
- Roll back silently if the tap sync fails; show stale banner

---

## Competitive Analysis

### WeMod

**What works well:**

- One-click trainer activation concept (not applicable to CrossHook which links out, but informative)
- Option count displayed upfront on each trainer listing ("+19 options") — sets expectations before clicking
- Category browsing by game genre
- Version history accessible — users can pin a specific trainer version

**What CrossHook can adopt:**

- Prominent option count in result cards
- Version pinning concept: show when a trainer has multiple tracked versions

**Pain points (not to replicate):**

- UI locks the games library sidebar at full width, even in fullscreen — wastes space. CrossHook's collapsible sidebar already avoids this.
- Free tier hides controller-toggle features — irrelevant for CrossHook but good reminder: don't gate core discovery behind a paywall concept

### Nexus Mods App

**What works well:**

- Tabs separating "Mods" from "Collections" in the library — clean separation of content types
- "Update All" in-place updating for version management
- File conflict view with drag-and-drop priority resolution

**What CrossHook can adopt:**

- Separate tabs for "Search Results" vs "Subscribed Sources" in the discovery panel
- Version update awareness: if a cached trainer has a newer version available, show an "Update available" badge

**Pain points (not to replicate):**

- Community-reported lack of automated game-version/mod-version compatibility checking — CrossHook's version matching is a direct competitive differentiator here
- Game version compatibility is the top feature request on their feedback board (confidence: Medium, per forum discussions) — confirming users care deeply about this signal

### ProtonDB

**What works well:**

- Tiered compatibility tiers (Platinum → Borked) with consistent color semantics — already adopted in CrossHook via `crosshook-protondb-tier-badge`
- Community reports with `supporting_report_count` surfaced inline — builds confidence
- Stale cache handling with clear "Showing cached" indicator
- Browser extension that embeds compatibility data inline into the Steam store

**What CrossHook can adopt:**

- The "N supporting reports" pattern already exists in `ProtonDbLookupCard` — apply the same to trainer community data
- "Background refresh failed" banner pattern (already implemented as `--stale` modifier) — reuse for trainer discovery

**Unique insight:** ProtonDB's biggest UX strength is reducing anxiety before a user installs — showing "1247 reports, Platinum" tells the user others have validated this. Trainer discovery should surface community report counts similarly: "Reported working by N users in [tap source]".

### FLiNG Trainer / Trainer Sites

**What works well:**

- Game version clearly displayed per trainer: "v1.0–v1.02+" — the primary thing users check
- Last updated date visible per trainer — freshness signal
- Option count per trainer upfront
- Alphabetical A–Z index for browsing when search fails

**What CrossHook can adopt:**

- Version range format: display as "v1.0–v1.02+" (compressed, human-readable). Compare against installed version and show compatibility badge.
- Date-based freshness signal per result card

**What CrossHook avoids (legal/security):**

- These sites host trainer binaries directly. CrossHook links to sources only — the version matching logic must work from metadata, not from downloading and scanning binaries.

### PCGamingWiki

**Key lesson:** PCGamingWiki is data-rich but famously hard to search. Their own community calls it "one of the most difficult sites to find games that meet certain criteria." CrossHook must avoid the same trap: **rich metadata that is impossible to surface is worthless**. Search must be fast, fuzzy-tolerant, and return contextual results, not just exact matches.

---

## Recommendations

### Must Have

1. **Debounced search (300ms)** with immediate local cache filtering — users expect instant feedback; remote fetch can follow
2. **Version compatibility badge (green/yellow/red/unknown)** per result, comparing trainer version range against installed game version — this is the top user need differentiating CrossHook from manual web searching
3. **External link via Tauri `open()`** — never attempt to download or execute trainers directly within CrossHook
4. **Stale cache banner** following the existing `--stale` modifier pattern — users must know when they are looking at outdated data
5. **Offline mode handling** — show cached results with an offline indicator; the "Trainer source index is unavailable" message pattern mirrors ProtonDB's existing implementation
6. **Zero-results state with guidance** — never leave users with a blank screen; always suggest "Browse community taps" or "Refine your search"
7. **Source trust indicator** distinguishing subscribed community taps from unverified sources

### Should Have

8. **Option count tag** per trainer card ("+19 cheat options") — sets user expectations before they follow a source link
9. **Background refresh indicator** (small spinner, non-blocking) so users know the index is updating without interrupting their browsing
10. **Skeleton loading cards** on initial panel open using the existing `crosshook-skeleton` class and `--crosshook-skeleton-duration` variable
11. **"Update available" badge** when a cached trainer entry has a newer indexed version — parallels Nexus Mods' update pattern
12. **Community report count** ("Reported by N users") inline in result cards — mirrors `ProtonDbLookupCard`'s `supporting_report_count` display
13. **Pre-fill game from active profile** when opening discovery from within a profile — reduces friction in the primary workflow

### Nice to Have

14. **Version history per trainer** — let users see when a trainer last changed and what version range it previously covered
15. **A–Z browse index** as a fallback when search returns zero results — FLiNG pattern for when the user does not know the exact game title
16. **"Find trainers for this game ↗" shortcut** in the existing TrainerSection component — opens discovery panel pre-filtered
17. **Pinned trainer version** — user can lock to a specific indexed version even if a newer one is available

---

## Open Questions

### Resolved

1. **Search scope:** Confirmed — tap results load from SQLite immediately (synchronous, no spinner). External search requires an explicit "Search Online" button. This aligns with the existing ProtonDB Refresh pattern.

2. **Offline behavior:** Confirmed — if taps synced before, search works fully offline from local FTS index. If never synced, empty state with "Add Community Tap" CTA. Version correlation returns `Untracked` / `unknown` when version snapshots unavailable.

3. **Trust levels:** Two-tier model confirmed — tap results (community-curated) vs. external results. No cryptographic signing infrastructure exists, so trust is determined by source type, not signature verification.

4. **Dual CTA:** Confirmed — "Import Profile" is primary when a community profile exists; "Get Trainer ↗" is always present when source_url is available. Results without a community profile show only "Get Trainer ↗".

5. **SHA-256 display:** Resolved as "Verified by community" label with collapsible raw hash via `CollapsibleSection`. Non-technical users see the label; power users can expand and copy.

### Still Open

6. **WebKitGTK constraints:** Are there known rendering issues with the `crosshook-skeleton` shimmer animation under WebKitGTK? `LibraryCard` already uses `crosshook-skeleton` — if that renders correctly, trainer discovery can reuse it safely.

7. **"Search Online" scope:** Does the online search query all known trainer source APIs, or only community taps that the user has not yet subscribed to? The distinction affects what results appear in the online section and how the results list is segmented.

8. **Gamepad navigation order:** With `useGamepadNav.ts` in use, what is the focus order for the discovery panel? Search field → filter controls → results list → expanded card actions is the recommended order, but this needs validation against the existing gamepad nav implementation.

---

## Sources

- [WeMod App — New UI Discussion](https://community.wemod.com/t/new-ui-is-great/313866)
- [WeMod — About The New Interface](https://community.wemod.com/t/about-the-new-interface/327310)
- [Nexus Mods App Releases](https://github.com/Nexus-Mods/NexusMods.App/releases)
- [Nexus Mods — Show compatible game versions](https://feedback.nexusmods.com/posts/94/show-compatible-game-version-s-for-mods-collections)
- [ProtonDB](https://www.protondb.com/)
- [ProtonDB Community Extension](https://github.com/Trsnaqe/ProtonDB-Community-Extension)
- [FLiNG Trainer — All Trainers](https://flingtrainer.com/all-trainers/)
- [PCGamingWiki — Advanced Wiki Search Discussion](https://community.pcgamingwiki.com/topic/5682-advanced-wiki-search/)
- [Carbon Design System — Status Indicators](https://carbondesignsystem.com/patterns/status-indicator-pattern/)
- [Carbon Design System — Loading Pattern](https://carbondesignsystem.com/patterns/loading-pattern/)
- [NN/G — Skeleton Screens 101](https://www.nngroup.com/articles/skeleton-screens/)
- [Search UX Best Practices — Pencil & Paper](https://www.pencilandpaper.io/articles/search-ux)
- [Master Search UX — Design Monks](https://www.designmonks.co/blog/search-ux-best-practices)
- [Offline UX Design Guidelines — web.dev](https://web.dev/articles/offline-ux-design-guidelines)
- [Offline-First Architecture — Medium](https://medium.com/@jusuftopic/offline-first-architecture-designing-for-reality-not-just-the-cloud-e5fd18e50a79)
- [Designing for Trust — UI Patterns](https://medium.com/@Alekseidesign/designing-for-trust-ui-patterns-that-build-credibility-e668e71e8d47)
- [Debouncing for Search Performance — Medium](https://medium.com/@sohail_saifii/the-debouncing-technique-that-fixed-our-search-performance-292bb427e5e1)
- [Badge UI Design Best Practices — Mobbin](https://mobbin.com/glossary/badge)
- [More Accessible Skeletons — Adrian Roselli](https://adrianroselli.com/2020/11/more-accessible-skeletons.html)
