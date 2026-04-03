# UX Research: Proton App ID — Art Management & Steam App ID Entry

**Confidence**: High (multi-source, competitive analysis, codebase analysis)
**Date**: 2026-04-02
**Topic velocity**: Moderate — game launcher UI patterns are relatively stable; file upload patterns evolve faster

---

## Executive Summary

This research covers UX patterns for two tightly coupled features:

1. **Steam App ID entry** on `proton_run` profiles — how users discover, enter, and validate an optional `steam_app_id` for media/metadata lookup.
2. **Tri-art system** — cover (landscape, 2.14:1), portrait (2:3), and background (wide, 16:9) art types per profile, with per-type custom upload layered over auto-downloaded art.

Key findings:

- **Search-as-you-type** (game name → App ID) is the dominant discovery pattern; direct numeric input is the power-user path. Both must be supported.
- **Art priority**: custom upload > auto-downloaded > placeholder initials. This matches Playnite, SteamGridDB, and GameVault conventions and is already partially implemented in `useGameCoverArt` (line 90: `customUrl ?? coverArtUrl`).
- **Per-type custom upload** is the recommended UX for the three art types; a tabbed or labeled media section in the profile editor surfaces each type individually. A "mix and match" model (custom portrait + auto cover + auto background) requires per-slot visual feedback on which source is active.
- **File picker + drag-drop** is the expected upload pattern for desktop apps. Native Tauri dialog is the correct vehicle.
- **Skeleton loading** on the Library grid is already implemented for the portrait slot (`useGameCoverArt` + `.crosshook-skeleton`); the background slot and profile detail view require the same treatment for new art types.
- **Invalid App ID** and **failed download** errors need inline, non-blocking feedback — not modal dialogs. A small inline badge near the field ("Not found on Steam") suffices.
- **Codebase-confirmed**: `MediaSection.tsx` currently shows only a path string — no inline thumbnail preview. Adding per-slot thumbnail previews is a must-have UX improvement (path-only display is the #1 pain point in Lutris). The `LibraryCard` portrait slot is already correct and does not need changes.
- **Open data model decision**: whether `proton_run` art uses a new `runtime.steam_app_id` field or the existing `steam.app_id` field affects UX label copy and section layout. See Section 2.1 for both options and their UX implications.

---

## User Workflows

### 1.1 Primary Flows

#### A. Adding Steam App ID to a Proton Run Profile (Primary Discovery Flow)

The user has an existing `proton_run` profile with a game that has a known Steam presence but launches directly via Proton (not `steam_applaunch`). They want cover art and metadata.

**Recommended flow: Search-as-you-type lookup**

```
Profile editor → Runtime section → "Steam App ID (optional)" field
  → User begins typing game name or numeric App ID
  → After 300 ms debounce: inline suggestions dropdown appears
    [ Dark Souls III         — 374320 ]
    [ Dark Souls II          — 236430 ]
    [ Dark Souls: Remastered — 570940 ]
  → User selects suggestion
  → Field populates with numeric App ID
  → Art downloads in background; Library card updates async
```

**Alternative: Direct numeric entry (power-user path)**

```
Profile editor → "Steam App ID (optional)" field
  → User pastes "374320"
  → Field validates on blur: shows green checkmark if found, red warning if not
  → Art downloads in background if valid
```

**Alternative: Paste Steam store URL**

```
  → User pastes "https://store.steampowered.com/app/374320/Dark_Souls_III/"
  → Field extracts "374320" automatically, confirms extraction inline
```

This URL-parsing pattern is used by Heroic Launcher and is particularly useful because users often have the Steam store page open.

**Confidence**: High — SteamDB and SteamGridDB both confirm this is how users look up App IDs; Heroic sideload art issues confirm numeric-only input is a friction point.

#### B. Custom Art Upload — Per Art Type

```
Profile editor → Media section → three labeled slots:
  [ Cover Art (landscape 2:1)  ] [Browse] [Clear]
  [ Portrait Art (2:3)         ] [Browse] [Clear]
  [ Background Art (16:9 wide) ] [Browse] [Clear]

  → User clicks "Browse" for Portrait Art
  → Native file picker opens (Tauri dialog plugin, image filter)
  → User selects file
  → File is imported to app data dir (Rust: import_custom_art)
  → Slot shows thumbnail preview immediately (optimistic)
  → Library card updates its portrait art slot
```

The current `MediaSection` (`src/crosshook-native/src/components/profile-sections/MediaSection.tsx`) supports one slot (`custom_cover_art_path`). This must expand to three named slots.

#### C. Clearing Custom Art (Reverting to Auto-Downloaded)

```
  → User clicks "Clear" for Portrait Art slot
  → Slot reverts to auto-downloaded art (if steam_app_id is set) or placeholder
  → Slot preview area shows the auto-downloaded art with a badge "Auto"
```

Clearing must not require a Save — it should apply immediately with optimistic update. If no auto-downloaded art exists, revert to initials fallback.

#### D. Mixed Art State (Custom + Auto)

The tri-art system allows per-slot independence. For example: custom portrait + auto cover + auto background.

```
Media section shows source badge on each slot:
  [ Cover Art   ] [ cover.png ]  [Custom]    [Browse] [Clear]
  [ Portrait    ]                [Auto]      [Browse] [Clear]
  [ Background  ]                [Not Set]   [Browse]
```

The source badge (Custom / Auto / Not Set) tells users at a glance what populates each slot without having to check the Library grid.

**Confidence**: High — Playnite's per-type media editing tab directly validates this per-slot source model.

### 1.2 Alternative Flows

| Flow                         | Trigger                                        | Resolution                                                                                      |
| ---------------------------- | ---------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| App ID not found on Steam    | User enters invalid ID or name with no match   | Inline warning: "No Steam game found for this ID" — non-blocking; art stays as current fallback |
| Art download fails (network) | Valid App ID but Steam/SteamGridDB unreachable | Skeleton → fallback initials; small retry icon in Library card; no modal                        |
| Corrupt image upload         | User selects unreadable file                   | Toast: "Could not read image file. Please try a different file."                                |
| Wrong aspect ratio upload    | User uploads portrait art in landscape slot    | Warn (non-blocking): "This image looks like portrait art. It will be cropped/letterboxed."      |
| No steam_app_id set          | proton_run profile without App ID              | Art slots show "No App ID set" badge; Browse still works for custom upload                      |
| User pastes Steam store URL  | steam.steampowered.com/app/NNNNN/...           | Auto-extract App ID, populate field, show confirmation badge                                    |

---

## 2. UI/UX Best Practices

### 2.1 Steam App ID Field — Input UX

#### Field Placement

The `steam_app_id` field for `proton_run` profiles belongs in the **Runtime section**, specifically in the Proton Runtime block — consistent with where `steam_applaunch` profiles show `app_id`. `RuntimeSection.tsx` already has a `Steam App ID` row for `proton_run` (lines 182–191), currently writing to `steam.app_id` with placeholder "Optional for ProtonDB lookup".

**Data model decision (open — needs team-lead resolution):**

**Option A — Unified field (reuse `steam.app_id`)**: Update the existing field's label to "Steam App ID (ProtonDB & art lookup)" and helper text to "Used for ProtonDB compatibility lookup and art download. Does not affect game launch." This is simpler (one field, one value) but conflates ProtonDB and art concerns.

**Option B — Separate field (`runtime.steam_app_id`)**: Add a distinct `runtime.steam_app_id` field for art-only lookup. The existing `steam.app_id` field retains its ProtonDB label unchanged. The Runtime section for `proton_run` would show both:

```
Steam App ID (optional)              [existing — ProtonDB lookup]
Steam App ID for Art (optional)      [new — art/metadata download only]
```

UX recommendation: **Option A** if the team wants minimal surface area; **Option B** if ProtonDB and art App IDs can legitimately differ for a game (e.g., a user has entered an incorrect ProtonDB App ID and wants art from a different one). Either option is workable — the label and helper text fully communicate the scope difference.

**Regardless of option chosen**, the label must explicitly state "Does not affect game launch" since `proton_run` users who see an App ID field may assume it controls the Proton launch.

#### Autocomplete Pattern

A search-as-you-type combobox (not a plain text field) improves discoverability:

- **Trigger**: if input contains only digits → validate as App ID; if contains letters → fuzzy search Steam app list
- **Debounce**: 300 ms (avoids excessive API calls during typing)
- **Dropdown**: max 8 results, game title + App ID per row
- **Loading state**: inline spinner in field while suggestions are fetching
- **No results**: show "No results. Enter App ID directly." at bottom of dropdown
- **On selection**: field populates with numeric ID; dropdown closes; art download starts

**Data source**: Steam API `IStoreService/GetAppList` or a locally cached app list. The api-researcher teammate is researching this; integrate their findings.

#### Validation on Blur

After a numeric ID is entered without using autocomplete:

- Green checkmark + game name: `374320 — Dark Souls III` (confirmed via Steam API)
- Red warning: `Invalid Steam App ID` (no match found)
- Gray spinner during validation fetch

**Confidence**: High — Material UI Autocomplete, Steam-App-ID-Finder GitHub, and Heroic sideload art issues all confirm this pattern.

### 2.2 Art Slots — Media Section UX

#### Three-Slot Design

Replace the current single `custom_cover_art_path` FieldRow in `MediaSection.tsx` with three labeled slots. The current implementation shows only a file path string — no thumbnail. Each new slot should show:

1. **Thumbnail preview** at correct aspect ratio (replaces the path display; path shown as secondary small text below for power users)
2. **Source badge**: Custom / Auto / Not Set
3. **Browse button** (opens native file picker)
4. **Clear button** (visible only when Custom source is active; clears to Auto or Not Set)

Art type specifications:

| Slot                  | Aspect Ratio      | Use                     | Steam Spec       |
| --------------------- | ----------------- | ----------------------- | ---------------- |
| Cover Art (landscape) | ~2.14:1 (460×215) | Profile detail backdrop | Header capsule   |
| Portrait Art          | 2:3 (600×900)     | Library grid card       | Vertical capsule |
| Background Art        | ~16:9 or 3:1 wide | Future hero section     | Hero (920×430)   |

#### Aspect Ratio Handling

Each slot's thumbnail uses `object-fit: cover` at the correct aspect ratio. This ensures:

- Landscape cover art doesn't distort in the 2:1 preview
- Portrait art doesn't distort in the 2:3 preview

If a user uploads an image with mismatched aspect ratio (e.g., portrait image in landscape slot), the preview will show the cropped result immediately. This is the correct behavior — "What you see is what you get."

A soft warning (not a blocker): "This image has a portrait aspect ratio (2:3). It will be center-cropped to fit the landscape slot. For best results, use a wider image."

**Confidence**: High — Playnite media editing workflow, Steam art upload dimensions, and SteamTinkerLaunch artwork docs confirm per-slot aspect ratio management.

#### File Upload UX

Pattern: **file picker primary, drag-and-drop optional enhancement**

- File picker: native Tauri `dialog.open()` with filter `{name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp']}`
- Drag-and-drop: accept drops onto the slot thumbnail area; highlight border on dragover
- Paste: support Ctrl+V to paste clipboard images (desktop users frequently screenshot-paste art)
- Maximum accepted file size: 10 MB (warn above this; images above 10 MB are unusably large for thumbnails)

**Confidence**: High — Uploadcare UX best practices, Shadcn drag-drop uploader, and general file upload UX guidelines confirm this set of patterns.

### 2.3 Art Priority Visual Communication

When `steam_app_id` is set, users need to understand what art they'll see without navigating to the Library grid.

**Source Priority Display in Media Section:**

```
Cover Art (landscape)
  [thumbnail: shows current active image]
  Source: ● Custom  ○ Auto  ○ None
  [Browse] [Clear]
```

The "Source" radio-style indicator communicates the active source at a glance. It is read-only — users don't toggle it directly; it changes when they upload custom art (sets Custom) or click Clear (reverts to Auto if App ID is set).

**Priority chain visualization**: A short helper text below the section reads: "Custom art takes priority over auto-downloaded art. If no App ID is set, custom art is the only available source."

### 2.4 Library Grid — Portrait Art Integration

The Library grid already fetches portrait art via `useGameCoverArt(appId, customPath, 'portrait')` in `LibraryCard.tsx` (line 50). `LibraryCard.tsx` itself needs no changes.

The key requirement is that `profile_list_summaries` resolves the **art App ID** for each profile before returning it to the frontend:

- For `steam_applaunch` profiles: `steamAppId = steam.app_id`
- For `proton_run` profiles: `steamAppId = runtime.steam_app_id ?? steam.app_id` (if Option B above) or simply `steam.app_id` (if Option A)
- For `native` profiles: `steamAppId = None`

This resolution happens in Rust — `LibraryCard` receives a single `steamAppId?: string` and is unaware of which profile field it came from.

`LibraryCardData` will also need `customPortraitArtPath?: string` (separate from existing `customCoverArtPath`) so the portrait slot in the Library grid can use custom portrait art independently of the cover art used in the profile editor backdrop.

**No changes to `LibraryCard.tsx` rendering logic** — the IntersectionObserver, skeleton, and fallback initials patterns are correct and complete.

### 2.5 Accessibility

- **App ID field** — `aria-label="Steam App ID for art lookup (optional)"`, `role="combobox"` if autocomplete is added
- **Art slots** — each slot has a visible label; Browse button has `aria-label="Browse for [slot name] image"`; thumbnail has `alt="[slot name] preview"`
- **Error states** — use `aria-live="polite"` regions for inline validation messages so screen readers announce them without interrupting
- **Color alone** — never use color alone to signal error/success; pair with icon + text (checkmark/warning icon)

---

## 3. Error Handling UX

### 3.1 Error States Table

| State                                       | Trigger                                   | Visual Treatment                                                       | Blocking? |
| ------------------------------------------- | ----------------------------------------- | ---------------------------------------------------------------------- | --------- |
| Invalid App ID (numeric, not found)         | ID does not exist on Steam                | Inline red warning below field: "No Steam game found for ID [N]"       | No        |
| App ID name search, no results              | Name not in Steam database                | Dropdown: "No results. Enter ID directly."                             | No        |
| Art download failed (network)               | Steam/SteamGridDB unreachable             | Library card shows initials fallback; small retry icon on hover        | No        |
| Art download failed (invalid ID after save) | ID was valid at entry but art fetch fails | Skeleton → initials fallback; no visible error in editor               | No        |
| Corrupt / unreadable upload file            | File cannot be decoded as image           | Toast (3 s): "Could not read the selected file. Try a PNG or JPEG."    | No        |
| File too large (> 10 MB)                    | User selects oversized file               | Toast (3 s): "File is too large (max 10 MB). Resize and try again."    | No        |
| Upload import command fails (Rust side)     | `import_custom_art` IPC returns error     | Toast: "Failed to import image. Check that the file is a valid image." | No        |
| Mismatched aspect ratio                     | Portrait art in landscape slot            | Non-blocking warning badge in preview: "Image will be cropped"         | No        |

### 3.2 Validation Patterns

**App ID field validation sequence:**

1. User types → no validation feedback while typing (avoid distracting red flashes)
2. User stops typing 300 ms → autocomplete suggestions appear (letters) or validation fetch starts (digits)
3. User blurs field → show result: green checkmark + game name, or red warning + message
4. On save → re-validate if field changed; do not block save for an invalid App ID (it's optional)

**Why non-blocking on save?** App ID is optional. If the Steam service is offline at save time, blocking save would be a poor UX. Art will simply stay as the fallback until the download succeeds.

**Image upload validation sequence:**

1. File selected → check format and size client-side (before IPC call)
2. If invalid format or too large → show immediate toast, do not call IPC
3. If valid → call `import_custom_art` → show loading indicator on thumbnail slot → on success, update preview

### 3.3 Error Message Security

Do not expose:

- Internal file paths in user-facing error messages
- Raw Rust error strings (e.g., `std::io::Error: permission denied`)
- Steam API response bodies

Map internal errors to user-friendly strings before display. Example: `io::Error` → "Could not save the image. Check file permissions."

**Confidence**: High — existing `MediaSection.tsx` already has a security-aware pattern (falls back to raw path but logs the error; the new implementation should not expose the internal error message at all).

---

## Performance UX

### 4.1 Art Download Loading States

When the user sets a new `steam_app_id` (or saves a profile that adds one), the art download happens in the background. The UX must communicate this without blocking the editor:

1. **Profile editor**: No loading state in the editor after save. Art downloads asynchronously.
2. **Library grid**: Card shows skeleton shimmer (existing `.crosshook-skeleton`) until art is available. When art loads, it fades in (opacity 0 → 1, 150 ms).
3. **Media section preview**: On first load of the editor with a valid App ID, each slot shows skeleton if art is not yet cached, then transitions to the image.

**Confidence**: High — existing `LibraryCard.tsx` already implements IntersectionObserver + skeleton + fade pattern; same pattern applies to new art slots.

### 4.2 Autocomplete Performance

For game name search:

- Throttle/debounce at 300 ms to avoid excessive API calls
- Cache previous results in a `useRef` map to avoid redundant fetches for the same prefix
- If using a local cached app list: fuzzy search is CPU-bound; run on a Web Worker or invoke via Tauri command to avoid blocking the UI thread
- The local app list (all ~170k Steam apps) is ~10 MB JSON — only cache after first fetch; do not bundle in app binary

**Confidence**: Medium — Steam app list size is documented; Web Worker pattern for fuzzy search is standard; specific sizing for CrossHook's expected network depends on api-researcher findings.

### 4.3 Image Thumbnail Preview

After custom art upload, show the thumbnail immediately (optimistic) before the Rust `import_custom_art` command completes:

1. On file selection → read file as ObjectURL → show preview immediately
2. Fire `import_custom_art` in background → on success, replace ObjectURL with `convertFileSrc(importedPath)` → revoke ObjectURL
3. On failure → revert preview to previous state, show error toast

This avoids a loading state for the preview (disk imports are fast) while keeping the displayed path consistent with the imported path.

**Confidence**: High — standard optimistic file preview pattern; validated by Uploadcare UX best practices.

### 4.4 Art Cache Invalidation

When a user clears custom art or changes the `steam_app_id`, the `useGameCoverArt` hook already reacts to prop changes via `useEffect`. No additional cache invalidation is needed in the hook itself. The Rust backend caches downloaded art per App ID per type — this cache does not need to be invalidated on App ID change (the new ID will fetch fresh art to a different cache key).

---

## 5. Competitive Analysis

### 5.1 Steam — Custom Art Management

Steam allows per-game custom art via right-click → Manage → Set Custom Artwork. Users can set: Hero, Logo, Cover (vertical capsule), Header capsule, and Tenfoot (Big Picture).

**What works**:

- Per-slot custom art with clearly labeled dimensions in the dialog
- SteamGridDB community ecosystem for pre-made art
- Custom art persists per-installation locally

**What doesn't work**:

- No search-to-find App ID UI (you're already in Steam, so ID is implicit)
- Custom art dialog is buried in right-click → Manage → Set Custom Artwork
- No visual source badge to distinguish custom vs. official art

**Confidence**: High — steamtinkerlaunch wiki, Steam community discussions, Steamworks documentation.

### 5.2 Playnite — Per-Type Media Editing

Playnite has the most mature multi-type art system among open launchers. The Game Edit dialog has a **Media tab** with:

- Cover image slot
- Background image slot
- Icon slot
- Each slot has a globe icon for web image search + file browse

**What works**:

- Media tab isolates art management from metadata/settings
- Web image search opens inline within the app (globe icon per slot)
- Supports setting cover as background fallback
- Users can batch-edit art across multiple games

**What doesn't work**:

- Takes 5 actions to change cover art (reported user friction in GitHub issue #3545)
- No quick action from Library grid directly — must go through full edit dialog
- Globe search opens a browser-style image picker that feels disconnected from the editor

**Learnings for CrossHook**:

- Media section tab approach is the right model, but add quick-access from Library card (Edit → jump to Media tab)
- Inline thumbnail previews avoid the "disconnected picker" feel

**Confidence**: High — Playnite documentation, GitHub issues, user reviews.

### 5.3 Heroic Games Launcher — Sideloaded Game Art

Heroic uses SteamGridDB as its primary art source. For sideloaded games, the current flow requires editing `~/.config/heroic/sideload_apps/library.json` directly — no GUI for custom art on sideloaded entries.

GitHub issue #4821 ("Sideloaded games cover art improvements") proposes:

- Separate tall + wide cover image support
- Post-addition editing via GUI (not file editing)
- Multiple search results to browse, not just first match

**Learnings for CrossHook**:

- The manual JSON editing approach is a clear pain point — CrossHook must not require file editing for art management
- Separate art type slots (tall/wide) is a recognized need
- Browse-multiple-results is preferred over auto-selecting first result

**Confidence**: High — GitHub issue #4821, Heroic community forums.

### 5.4 Lutris — Cover Art Management

Lutris covers are stored in `~/.cache/lutris/coverart/` as JPEG files. Custom art is set by clicking the cover image in the game configuration. Community art comes from lutris.net.

**What works**:

- Click-to-replace on the art preview in the editor is a discoverable pattern
- Community database with contribution path

**What doesn't work**:

- Multiple art types not supported (only one cover art per game)
- No inline validation or preview before saving
- Contribution path is unclear (Discord-only for cover art)

**Learnings for CrossHook**:

- "Click the art preview to replace" is a discoverable interaction for custom upload — pair this with a Browse button for discoverability
- Multiple types require labeled slots (Lutris's single-slot model doesn't scale to tri-art)

**Confidence**: High — Lutris forums, GitHub issues #4213, #4524.

### 5.5 SteamGridDB — Art Browsing UX

SteamGridDB provides a web and API-based art browser. The Decky Loader plugin for Steam Deck brings this inline.

**What works**:

- Browsing multiple art options per game before selecting
- Art type filtering (grids, heroes, logos, icons)
- Resolution and style filters
- Upload contribution flow

**What doesn't work**:

- Web-first interface feels out of place in desktop launcher context
- No local app integration (art management is via Steam, not in SteamGridDB itself)

**Learnings for CrossHook**:

- Future enhancement: inline SteamGridDB browse per slot (select from multiple options, not just upload local file)
- For V1: auto-download best-match from SteamGridDB API is sufficient; browse is a V2 enhancement

**Confidence**: High — SteamGridDB website, Decky plugin GitHub.

### 5.6 Competitive Summary

| Feature                      | Steam          | Playnite      | Heroic      | Lutris      | CrossHook (current)        |
| ---------------------------- | -------------- | ------------- | ----------- | ----------- | -------------------------- |
| Multiple art type slots      | Yes (5 types)  | Yes (3 types) | No (1 type) | No (1 type) | No (1 type)                |
| Source badge (custom/auto)   | No             | No            | No          | No          | No                         |
| Search by game name → App ID | N/A (implicit) | Via plugin    | No          | No          | No (yet)                   |
| File picker + drag-drop      | Yes            | Yes           | Partial     | Minimal     | Partial (file picker only) |
| Inline thumbnail preview     | Yes            | Yes           | No          | No          | No                         |
| Non-blocking art errors      | Yes            | Yes           | Partial     | Partial     | Yes (silent fallback)      |
| Quick art access from grid   | No             | No            | No          | No          | No                         |

---

## 6. Recommendations

### Must Have (V1)

1. **Rename and annotate the App ID field on `proton_run`** — label as "Steam App ID (optional)", helper text clarifies art-only use, no launch impact.
2. **Three art slots in MediaSection** — Cover (2.14:1), Portrait (2:3), Background (16:9). Each slot has: thumbnail preview, source badge (Custom/Auto/Not Set), Browse button, Clear button.
3. **Priority chain**: custom → auto-downloaded → initials fallback. Implemented in `useGameCoverArt`; extend to portrait and background slots.
4. **Inline validation** on App ID field (on blur, not while typing) — green checkmark + game name if valid, red warning if not found.
5. **Non-blocking art errors** — all art failures silently fall back to initials; no modal dialogs.
6. **File picker with image filter** — Tauri dialog plugin, png/jpg/jpeg/webp; 10 MB size warning.
7. **Optimistic thumbnail preview** — show thumbnail immediately after file selection; import in background.
8. **Helper text on MediaSection** — explain that Art Source (Auto) requires a Steam App ID to be set in the Runtime section.

### Should Have (V1 enhancement)

9. **Aspect ratio soft warning** — non-blocking: "This image will be cropped to fit the [slot] slot." when detected mismatch.
10. **App ID autocomplete** — search-by-game-name combobox; depends on api-researcher confirming Steam search API availability.
11. **URL paste auto-extraction** — detect `store.steampowered.com/app/NNNNN/` in App ID field and extract numeric ID.
12. **"Quick Edit Media" shortcut from Library card** — Edit button on LibraryCard navigates to ProfileEditor with Media tab pre-focused.
13. **Source badge on Library card** — small "Custom" badge in corner of Library card when custom portrait art is active (differentiates from auto art; helps debug mismatch).

### Nice to Have (V2+)

14. **Inline SteamGridDB art browse** — per slot, show multiple options from SteamGridDB API; user picks best fit.
15. **Drag-and-drop onto art slot** — drop an image file directly onto the slot thumbnail area.
16. **Clipboard paste** (Ctrl+V) — paste clipboard image directly into focused art slot.
17. **Bulk art refresh** — "Re-download art" action on Library toolbar to refresh auto art for all profiles with App IDs set.
18. **Art preview in context** — small "Preview in Library" link below the Media section that shows how the portrait art looks in the grid.

---

## 7. Open Questions

| Question                                                                                                | Status                                                                                                     | Why it matters                                                                               | Owner          |
| ------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- | -------------- |
| Single `steam.app_id` field (Option A) vs. separate `runtime.steam_app_id` (Option B) for `proton_run`? | **Open — team-lead decision**                                                                              | Drives label copy, field count in Runtime section, and Rust data model                       | team-lead      |
| Will autocomplete search use a live Steam API or local cached app list?                                 | Open — api-researcher researching                                                                          | Affects debounce strategy, offline behavior, and whether a Tauri command is needed           | api-researcher |
| Should Background art slot be exposed in V1 (no hero section yet)?                                      | Open                                                                                                       | Avoids dead UI if there is no consumer for background art in V1                              | team-lead      |
| What is the `profileId` key used to name imported custom art files in the managed media dir?            | Open — tech-designer to confirm                                                                            | `import_custom_art(source_path, art_type)` needs a stable identifier for the import filename | tech-designer  |
| Error UX for `import_custom_cover_art` failure: currently silent fallback to raw path                   | **Needs fix** — current `MediaSection.tsx` catch block stores unimported path; new implementation must not | Security and correctness concern; should become a toast + no path stored                     | tech-designer  |

---

## Sources

- [Playnite — Game Metadata Documentation](https://api.playnite.link/docs/manual/library/games/metadata.html)
- [Heroic Games Launcher — Sideloaded cover art improvements (GitHub issue #4821)](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/issues/4821)
- [Heroic Games Launcher — Support loading local image files (GitHub issue #2277)](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/2277)
- [Lutris Forum — How do I add cover art?](https://forums.lutris.net/t/how-do-i-add-cover-art-to-a-game/21480)
- [Lutris GitHub — Menu to select custom cover art (issue #4213)](https://github.com/lutris/lutris/issues/4213)
- [SteamTinkerLaunch — Custom Game Artwork Wiki](https://github.com/sonic2kk/steamtinkerlaunch/wiki/Custom-Game-Artwork)
- [Steam Graphical Assets — Steamworks Documentation](https://partner.steamgames.com/doc/store/assets)
- [Steam Library Assets — Steamworks Documentation](https://partner.steamgames.com/doc/store/assets/libraryassets)
- [SteamGridDB — Home](https://www.steamgriddb.com/)
- [SteamGridDB Decky Plugin — GitHub](https://github.com/SteamGridDB/decky-steamgriddb)
- [Steam-App-ID-Finder — GitHub](https://github.com/NikkelM/Steam-App-ID-Finder)
- [UX Best Practices for File Uploader — Uploadcare](https://uploadcare.com/blog/file-uploader-ux-best-practices/)
- [Drag-and-Drop UX Guidelines — Smart Interface Design Patterns](https://smart-interface-design-patterns.com/articles/drag-and-drop-ux/)
- [Optimistic UIs in Under 1000 Words — UX Planet](https://uxplanet.org/optimistic-1000-34d9eefe4c05)
- [6 Loading State Patterns That Feel Premium — UX World](https://medium.com/uxdworld/6-loading-state-patterns-that-feel-premium-716aa0fe63e8)
- [Game Library (library-home) — UX Research](../library-home/research-ux.md) (cross-reference: Library grid patterns and skeleton loading already established)
- [Tauri v2 File System Plugin](https://v2.tauri.app/plugin/file-system/)
- [Tauri v2 Upload Plugin](https://v2.tauri.app/plugin/upload/)
- [GameVault — Metadata Enrichment (priority/fallback model)](https://gamevau.lt/docs/server-docs/metadata-enrichment/metadata/)
- [Empty State UX — LogRocket](https://blog.logrocket.com/ux-design/empty-state-ux/)
- [Playnite Quick Action Change Cover (GitHub issue #3545)](https://github.com/JosefNemec/Playnite/issues/3545)
