# Library-Home: Documentation Research

## Overview

All documentation needed to implement library-home exists and is comprehensive. The feature spec (`feature-spec.md`) is the single authoritative source for resolved decisions. Seven research files cover external APIs, business rules, technical architecture, UX patterns, security, engineering practices, and recommendations. Key code files are well-commented; the most important inline constraints are in `useGameCoverArt.ts` (hardcodes `imageType: 'cover'` — must be extended to accept `'portrait'`) and `ContentArea.tsx` (uses a TypeScript `never` exhaustive guard that will cause a compile error until `'library'` is added to `AppRoute`).

---

## Architecture Docs

| Document | Path | What It Covers |
|---|---|---|
| Feature Spec (master) | `docs/plans/library-home/feature-spec.md` | Complete resolved spec: component tree, data models, IPC table, CSS variables, phasing, persistence classification |
| Technical Research | `docs/plans/library-home/research-technical.md` | Architecture design, data flow, API design, codebase change list (files to create and modify), confirmed decisions |
| Architecture Research | `docs/plans/library-home/research-architecture.md` | System structure analysis — component hierarchy, data flow, integration points |
| Stack Overview | `AGENTS.md` §Stack Overview, §Directory Map | Layer-by-layer tech stack; directory map for `src-tauri/`, `crates/crosshook-core/`, `src/` |
| Route Wiring | `src/crosshook-native/src/components/layout/ContentArea.tsx:34` | `switch(route)` dispatch; uses `never` exhaustive guard — **adding `'library'` to `AppRoute` is required before `ContentArea` will compile** |
| Route Type | `src/crosshook-native/src/components/layout/Sidebar.tsx:13` | `AppRoute` union type; `SIDEBAR_SECTIONS` array defines sidebar navigation items |
| Default Route | `src/crosshook-native/src/App.tsx:43` | `useState<AppRoute>('profiles')` — change to `'library'`; `VALID_APP_ROUTES` record at line 19 |
| SQLite Schema | `AGENTS.md` §SQLite Metadata DB | Table inventory (v13, 18 tables); `profiles.is_favorite` column used by favorites; `game_image_cache` keyed by `(steam_app_id, image_type)` |

---

## API Docs

### IPC Commands (Frontend → Backend)

| Command | Signature | File | Notes |
|---|---|---|---|
| `profile_list` | `() → Vec<String>` | `src-tauri/src/commands/profile.rs` | Called via `refreshProfiles()` on mount |
| `profile_list_favorites` | `() → Vec<String>` | `src-tauri/src/commands/profile.rs` | Read from `ProfileContext.favoriteProfiles` |
| `profile_set_favorite` | `{ name, favorite } → ()` | `src-tauri/src/commands/profile.rs` | Called via `toggleFavorite()` |
| `profile_load` | `{ name } → GameProfile` | `src-tauri/src/commands/profile.rs` | Used via `selectProfile()` before navigation |
| `fetch_game_cover_art` | `{ appId, imageType? } → Option<String>` | `src-tauri/src/commands/game_metadata.rs` | Accepts `imageType: 'cover'` today; must accept `'portrait'` |

### New IPC Command (Required)

| Command | Signature | Purpose |
|---|---|---|
| `profile_list_summaries` | `() → Vec<ProfileSummary>` | Batch-reads all TOMLs server-side; returns `{name, game_name, steam_app_id, custom_cover_art_path}[]` in one round-trip |

See `research-technical.md` for the full Rust struct definition and implementation notes.

### Rust Game Images Module

| File | Purpose |
|---|---|
| `src/crosshook-native/crates/crosshook-core/src/game_images/models.rs` | `GameImageType` enum — add `Portrait` variant here |
| `src/crosshook-native/crates/crosshook-core/src/game_images/client.rs` | `build_download_url` and `filename_for` — add `Portrait` arm |
| `src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs` | `build_endpoint` — add `Portrait` arm with `dimensions=342x482,600x900` |

### External APIs

| API | Base URL | Auth | Notes |
|---|---|---|---|
| Steam CDN (portrait) | `cdn.cloudflare.steamstatic.com/steam/apps/{APP_ID}/library_600x900_2x.jpg` | None | 2x = 600×900; 1x fallback = `library_600x900.jpg`; `header.jpg` as last resort; 404 expected for old titles |
| SteamGridDB | `steamgriddb.com/api/v2` | API key (90-day rotation) | Already integrated in `steamgriddb.rs`; no new integration needed |

Full external API research: `docs/plans/library-home/research-external.md`

---

## Development Guides

### Project Conventions

| Document | Path | Key Rules for This Feature |
|---|---|---|
| Agent Rules | `CLAUDE.md` | IPC `snake_case` names; Serde on all boundary types; business logic in `crosshook-core` not `src-tauri`; Conventional Commits |
| Stack Guidelines | `AGENTS.md` §SHOULD | `PascalCase` components; `camelCase` hooks; wrap `invoke()` in hooks; BEM `crosshook-*` classes; use `layout.css` scroll shell classes |
| Engineering Practices | `docs/plans/library-home/research-practices.md` | Reusable code inventory; KISS assessment; interface design for `LibraryCardProps`; testability patterns |

### CSS and Layout

| Token / Class | File | Value / Purpose |
|---|---|---|
| `--crosshook-color-bg` | `variables.css:4` | `#1a1a2e` — dark background for fallback gradient |
| `--crosshook-color-bg-elevated` | `variables.css:5` | `#20243d` — second color for fallback gradient |
| `--crosshook-color-surface` | `variables.css:6` | `#12172a` — card background |
| `--crosshook-color-accent` | `variables.css:13` | `#0078d4` — accent blue for initials in fallback tile |
| `--crosshook-radius-md` | `variables.css:39` | `14px` — card border radius |
| `--crosshook-touch-target-min` | `variables.css:43` | `48px` — minimum button height |
| `--crosshook-community-profile-grid-min` | `variables.css:52` | `280px` — reference for the `auto-fill minmax()` grid pattern |
| Scroll shell | `layout.css` | `crosshook-page-scroll-shell--fill`, `crosshook-route-stack`, `crosshook-route-stack__body--scroll` — use these on LibraryPage, not one-off viewport chains |

**New CSS variables to add to `variables.css`:**
```css
--crosshook-library-card-width: 190px;
--crosshook-library-card-aspect: 3 / 4;
--crosshook-library-grid-gap: var(--crosshook-grid-gap);
```

### Hooks with Inline Documentation

| Hook | File | Inline Docs Summary |
|---|---|---|
| `useGameCoverArt` | `src/crosshook-native/src/hooks/useGameCoverArt.ts` | Race-condition-safe via `requestIdRef`; returns `{ coverArtUrl, loading }`; currently **hardcodes `imageType: 'cover'` at line 42** — add optional `imageType?` param |
| `useImageDominantColor` | `src/crosshook-native/src/hooks/useImageDominantColor.ts` | Canvas-based 32×32 downsample; top-third weighted for banner tint; luminance boost for dark colors; returns `[r, g, b] | null` |
| `useProfile` / `ProfileContext` | `src/crosshook-native/src/hooks/useProfile.ts` | `profiles: string[]`, `favoriteProfiles: string[]`, `selectProfile(name)`, `toggleFavorite(name, bool)`, `refreshProfiles()` — consume via `useProfileContext()` |
| `ProfileContext` JSDoc header | `src/crosshook-native/src/context/ProfileContext.tsx:1-8` | Documents context responsibilities and what each consumer should expect |
| `GameProfile` TypeScript interface | `src/crosshook-native/src/types/profile.ts` | Authoritative frontend schema for `GameProfile`; defines `steam.app_id`, `game.custom_cover_art_path`, and all other fields used by IPC |
| `game_images/client.rs` inline docs | `src/crosshook-native/crates/crosshook-core/src/game_images/client.rs` | Explains cache lifecycle and security rationale (magic-byte validation, `safe_image_cache_path`); read before adding `Portrait` variant |

---

## Must-Read Documents (Prioritized for Implementers)

Ordered by reading priority for someone implementing Phase 1:

1. **`docs/plans/library-home/feature-spec.md`** — Start here. Contains all resolved decisions, component tree, data models, IPC table, CSS variables, phase breakdown, and persistence classification. Skip the research files if you read only one document.

2. **`docs/plans/library-home/research-technical.md`** — Detailed architecture, exact files to create and modify (with line numbers), confirmed `GameImageType::Portrait` change, full API design with Rust code samples. Most implementation-critical details live here.

3. **`docs/plans/library-home/research-practices.md`** — Reusable code inventory, `LibraryCardProps` interface design, KISS assessment (no speculative tabs), testability patterns. Prevents redundant abstractions and wrong decomposition choices.

4. **`src/crosshook-native/src/hooks/useGameCoverArt.ts`** — Read before writing any cover art code. The `imageType` hardcode at line 42 is a required change. The `requestIdRef` cancellation pattern must be preserved.

5. **`src/crosshook-native/src/components/layout/ContentArea.tsx`** — Read before touching routes. The `never` exhaustive guard at line 50 will cause a TypeScript compile error until `'library'` is added to the `AppRoute` union.

6. **`docs/plans/library-home/research-ux.md`** — Competitive analysis, card design decisions (always-visible vs hover-only buttons), gradient scrim WCAG values, skeleton loading patterns, empty-state spec. Critical for correct visual implementation.

7. **`docs/plans/library-home/research-security.md`** — S-01/S-06 (`custom_cover_art_path` path traversal), S-12 (`profile_list_summaries` path sanitization), S-02 (CSP impact if CDN URLs ever render directly). Skim before finalizing IPC DTOs.

8. **`AGENTS.md`** — Stack overview and directory map. Reference when deciding where to add new files.

9. **`docs/plans/library-home/research-external.md`** — Steam CDN URL patterns, SteamGridDB API notes, virtual scrolling library comparison. Reference if implementing the Rust `Portrait` image type or evaluating TanStack Virtual.

10. **`docs/plans/library-home/research-business.md`** — User stories, business rules R1–R10, edge cases table. Reference if a behavioral question arises during implementation.

11. **`docs/plans/library-home/research-recommendations.md`** — Phasing rationale, technology decisions, and the cover art metadata tension (Options A/B/C). Resolved in the feature spec as Option A.

---

## Documentation Gaps

| Gap | Impact | Notes |
|---|---|---|
| No JSDoc on `useGameCoverArt` for the `imageType` parameter | Low | The parameter doesn't exist yet; document it when adding the optional `imageType?` arg |
| `layout.css` scroll shell class names are documented in `AGENTS.md` but the file has no inline comments | Low | Read `layout.css` directly and compare with `ProfilesPage.tsx` or `LaunchPage.tsx` as reference implementations |
| No frontend test framework configured | Low — no tests to write | `research-practices.md` §Testability covers structural patterns for when a test runner is added |
| `useProfile.ts:519` `toggleFavorite` optimistic update contract is informal | Medium | The hook does not guarantee revert-on-error; LibraryCard must manage its own optimistic boolean state and handle the IPC error boundary itself |
| `profile_list_summaries` Rust command does not yet exist | High — implementation blocker | Spec and research-technical.md define the full struct and behavior; implement first in `crosshook-core` before building `LibraryCard` |
| Portrait cover art fallback chain within Steam CDN not yet implemented | High — required Rust change | `client.rs` only fetches the first URL; multi-URL fallback for `library_600x900_2x → library_600x900 → header` needs new Rust logic |
