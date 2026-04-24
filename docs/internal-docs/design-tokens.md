# CrossHook Design Tokens

Source of truth: [`src/crosshook-native/src/styles/variables.css`](../../src/crosshook-native/src/styles/variables.css).
Enforced by: [`scripts/check-legacy-palette.sh`](../../scripts/check-legacy-palette.sh), run via [`scripts/lint.sh`](../../scripts/lint.sh) in CI.

This doc defines the palette-token contract established in Phase 2 of the Unified Desktop Redesign ([PRD](../prps/prds/unified-desktop-redesign.prd.md)) and the guardrails that keep it from drifting back to the legacy Microsoft-blue palette.

## Rule: no literal accent / background colors

Stylesheets under `src/crosshook-native/src/**` and component CSS-in-TSX **must reference a `--crosshook-color-*` token**, never a raw hex or rgba literal for any color that belongs to the palette (accent, background, surface, sidebar, titlebar, scrim, accent glow, desaturated status).

Literals that are always forbidden in this tree:

| Pattern                       | Replacement token                                              |
| ----------------------------- | -------------------------------------------------------------- |
| `#0078d4`                     | `var(--crosshook-color-accent)`                                |
| `#2da3ff`                     | `var(--crosshook-color-accent-strong)`                         |
| `#1a1a2e`                     | `var(--crosshook-color-bg)`                                    |
| `#20243d`                     | `var(--crosshook-color-bg-elevated)`                           |
| `#12172a`                     | `var(--crosshook-color-surface)`                               |
| `rgba(0, 120, 212, <alpha>)`  | `rgba(74, 125, 181, <alpha>)` or a `--crosshook-color-*` token |
| `rgba(45, 163, 255, <alpha>)` | `rgba(107, 163, 217, <alpha>)`                                 |

Ad-hoc one-off colors that don't belong to the palette (e.g. a gradient fade with `rgba(0, 0, 0, …)`, status-specific rgba tints like `rgba(74, 222, 128, 0.12)` for capability chips) are not covered by the sentinel and can remain literal if no token applies. Prefer extracting a token when the same value appears in more than two places.

## Token catalogue (default theme)

All tokens live in the default `:root { … }` block of `variables.css`. The `:root[data-crosshook-theme='high-contrast']` block is a **separate palette** (amber accent, high-contrast body) and is deliberately out of the Phase 2 sweep — it owns its own accent literals because accessibility targets differ from the calm-desktop palette.

### Shell surfaces

| Token                              | Value                   | Purpose                                       |
| ---------------------------------- | ----------------------- | --------------------------------------------- |
| `--crosshook-color-bg`             | `#181a24`               | App body background                           |
| `--crosshook-color-bg-elevated`    | `#1f2233`               | Elevated panel background                     |
| `--crosshook-color-surface`        | `#141620`               | Surface under panels (modal scrim base, etc.) |
| `--crosshook-color-surface-strong` | `#0c1120`               | Deeper sunken surface                         |
| `--crosshook-color-sidebar`        | `#10121c`               | Primary sidebar background                    |
| `--crosshook-color-titlebar`       | `#0c0e16`               | Titlebar / app frame                          |
| `--crosshook-color-surface-1`      | `#1a1d28`               | Row surfaces                                  |
| `--crosshook-color-surface-2`      | `#22263a`               | Raised cards / panels                         |
| `--crosshook-color-surface-3`      | `#2a2f48`               | Hover / pressed card state                    |
| `--crosshook-color-scrim`          | `rgba(8, 10, 18, 0.78)` | Modal / overlay scrim                         |

### Accent

| Token                             | Value                       | Purpose                                     |
| --------------------------------- | --------------------------- | ------------------------------------------- |
| `--crosshook-color-accent`        | `#4a7db5`                   | Primary steel-blue accent                   |
| `--crosshook-color-accent-strong` | `#6ba3d9`                   | Brighter accent (hover, focus rings)        |
| `--crosshook-color-accent-soft`   | `rgba(74, 125, 181, 0.16)`  | Soft accent fills (selected row, chip bg)   |
| `--crosshook-color-accent-glow`   | `rgba(107, 163, 217, 0.22)` | Ambient accent glow (hero gradients, focus) |

### Status (existing + desaturated siblings)

The existing `--crosshook-color-success | warning | danger` tokens remain in place for components that have not yet migrated. The `*-muted` siblings introduced in Phase 2 are the **calm-desktop** values; components opt in during their rework phases.

| Token                             | Value     | Purpose                    |
| --------------------------------- | --------- | -------------------------- |
| `--crosshook-color-success-muted` | `#5fb880` | Calm success (Phase 2+ UI) |
| `--crosshook-color-warning-muted` | `#d4a94a` | Calm warning (Phase 2+ UI) |
| `--crosshook-color-danger-muted`  | `#d77a8a` | Calm danger (Phase 2+ UI)  |

## Adding a new token

1. Define it inside the default `:root { … }` block in `variables.css`, grouped with logically-related tokens.
2. Reference it exclusively via `var(--crosshook-color-…)` — never re-duplicate the literal in consuming stylesheets.
3. If the token replaces a pattern the sentinel does not yet catch, either:
   - Update the pattern list in `scripts/check-legacy-palette.sh` **and** the **Rule** table above, or
   - Accept that the old literal is permitted for this specific case and add `/* allow: legacy-palette */` on the offending line with a comment-anchored reason.

New tokens should ship alongside the first consumer. Orphan tokens without a consumer accumulate as dead code.

## Suppression grammar

When a legacy literal genuinely has to live in the tree (pasted upstream fixture, test asset, or an intentional reference in a code comment that cannot be rewritten), the sentinel accepts two forms:

| File type                    | Suppression                                                   |
| ---------------------------- | ------------------------------------------------------------- |
| `.css`, `.module.css`        | `/* allow: legacy-palette */` on the same line as the literal |
| `.ts`, `.tsx`, `.js`, `.jsx` | `// allow: legacy-palette` on the same line as the literal    |

Always pair the suppression with a one-sentence reason in the comment body:

```css
background: rgba(0, 120, 212, 0.18); /* allow: legacy-palette — test fixture, asserted against upstream bundle */
```

If you find yourself adding a suppression, consider refactoring the literal into a token instead. Suppressions should be **rare** and each one should justify its own existence.

## High-contrast theme carve-out

`:root[data-crosshook-theme='high-contrast']` owns its own accent (`#facc15 / #f97316`) and deliberately retains distinct literals for accessibility reasons. The sentinel patterns above do not overlap with the high-contrast palette, so no explicit carve-out is required in the script — but be aware when editing that the high-contrast block is **not** subject to the Phase 2 token swap.

## How CI enforces this

`scripts/check-legacy-palette.sh`:

- `--help` / `-h` — prints usage and exits 0.
- `--list` — prints the legacy literal patterns, one per line, and exits 0.
- `--selftest` — writes a synthetic CSS file containing a literal and asserts the scanner detects it; exit 0 on success, 1 on failure.
- No flag — scans `src/crosshook-native/src/**` and exits 0 if clean, 1 with per-match diagnostics if any legacy literal is found.

`scripts/lint.sh`:

- `--legacy-palette` — runs the sentinel only.
- `--all` (and bare `./scripts/lint.sh`) — includes the sentinel alongside Rust, TypeScript, shell, and host-gateway checks.
- Scope flags (`--staged`, `--unstaged`, `--modified`) do not narrow the palette check; it always scans the full tree, because a literal introduced in an unmodified file would otherwise escape detection on a focused run.

`.github/workflows/lint.yml` runs `./scripts/lint.sh` on every PR, so regressions fail CI.

---

## Path note

The PRD (`unified-desktop-redesign.prd.md:143`) references `docs/internal/design-tokens.md`
but the canonical path is **`docs/internal-docs/design-tokens.md`** (this file).
The CI sentinel (`scripts/check-legacy-palette.sh:136`) and all cross-references use this path.

---

## Typography tokens

| Token                   | Value                                                                               | When to use                                            |
| ----------------------- | ----------------------------------------------------------------------------------- | ------------------------------------------------------ |
| `--crosshook-font-body` | `'Avenir Next', 'Segoe UI', 'Helvetica Neue', system-ui, -apple-system, sans-serif` | All non-code text. Never hardcode `font-family`.       |
| `--crosshook-font-mono` | `'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace`                   | Code, paths, IDs, environment values, terminal output. |

---

## Radius and shadow tokens

| Token                       | Value                             | When to use                                 |
| --------------------------- | --------------------------------- | ------------------------------------------- |
| `--crosshook-radius-lg`     | `20px`                            | Cards, panels, modals                       |
| `--crosshook-radius-md`     | `14px`                            | Buttons, inputs, chips                      |
| `--crosshook-radius-sm`     | `10px`                            | Badges, tags, small pills                   |
| `--crosshook-shadow-soft`   | `0 18px 40px rgba(0, 0, 0, 0.32)` | Elevated surfaces: modals, overlays, panels |
| `--crosshook-shadow-strong` | `0 28px 70px rgba(0, 0, 0, 0.42)` | Floating UI: palette, popover, tooltips     |

---

## Spacing and layout tokens

These tokens enforce layout consistency. Use them instead of ad-hoc `px` values.

| Token                               | Value   | Purpose                                           |
| ----------------------------------- | ------- | ------------------------------------------------- |
| `--crosshook-page-padding`          | `32px`  | Outer padding of every route page body            |
| `--crosshook-panel-padding`         | `20px`  | Interior padding of dashboard panels              |
| `--crosshook-card-padding`          | `28px`  | Interior padding of content cards                 |
| `--crosshook-grid-gap`              | `20px`  | Gap between grid items (library, dashboard grids) |
| `--crosshook-touch-target-min`      | `48px`  | Minimum tap target height (WCAG 2.5.5 AA)         |
| `--crosshook-touch-target-compact`  | `44px`  | Compact tap target (used in dense lists)          |
| `--crosshook-button-height-compact` | `44px`  | Standard button height                            |
| `--crosshook-transition-fast`       | `140ms` | Micro-interactions: hover, active state           |
| `--crosshook-transition-standard`   | `220ms` | Panel open/close, sidebar collapse                |
| `--crosshook-library-card-width`    | `190px` | Library grid card base width                      |
| `--crosshook-library-card-aspect`   | `3 / 4` | Library card aspect ratio                         |

Responsive overrides exist in `variables.css` `@media` blocks — see § Responsive overrides below.

---

## Capability indicator tokens

Used by host-readiness and health-check UIs to communicate tool status.

| Token                                             | Value                       | Meaning                         |
| ------------------------------------------------- | --------------------------- | ------------------------------- |
| `--crosshook-color-capability-available`          | `#4ade80`                   | Tool present and working        |
| `--crosshook-color-capability-available-bg`       | `rgba(74, 222, 128, 0.12)`  | Chip background for available   |
| `--crosshook-color-capability-available-border`   | `rgba(74, 222, 128, 0.28)`  | Chip border for available       |
| `--crosshook-color-capability-degraded`           | `#fbbf24`                   | Tool present but limited        |
| `--crosshook-color-capability-degraded-bg`        | `rgba(251, 191, 36, 0.12)`  | Chip background for degraded    |
| `--crosshook-color-capability-degraded-border`    | `rgba(251, 191, 36, 0.28)`  | Chip border for degraded        |
| `--crosshook-color-capability-unavailable`        | `#f87171`                   | Tool missing or broken          |
| `--crosshook-color-capability-unavailable-bg`     | `rgba(248, 113, 113, 0.12)` | Chip background for unavailable |
| `--crosshook-color-capability-unavailable-border` | `rgba(248, 113, 113, 0.28)` | Chip border for unavailable     |

---

## Pipeline connector tokens

Connector lines between launch pipeline nodes. Use `color-mix()` variants only — never
hardcode a hex for connector states.

| Token                                          | Value                                                                       |
| ---------------------------------------------- | --------------------------------------------------------------------------- |
| `--crosshook-color-pipeline-connector-success` | `color-mix(in srgb, var(--crosshook-color-success) 35%, transparent)`       |
| `--crosshook-color-pipeline-connector-active`  | `color-mix(in srgb, var(--crosshook-color-accent-strong) 40%, transparent)` |
| `--crosshook-color-pipeline-connector-error`   | `color-mix(in srgb, var(--crosshook-color-danger) 35%, transparent)`        |
| `--crosshook-color-pipeline-connector-waiting` | `color-mix(in srgb, var(--crosshook-color-warning) 40%, transparent)`       |

---

## Autosave indicator tokens

Eight tokens for the four autosave states × background/border.

| Token                                                                     | State               |
| ------------------------------------------------------------------------- | ------------------- |
| `--crosshook-autosave-saving-bg` / `--crosshook-autosave-saving-border`   | Save in progress    |
| `--crosshook-autosave-success-bg` / `--crosshook-autosave-success-border` | Save succeeded      |
| `--crosshook-autosave-warning-bg` / `--crosshook-autosave-warning-border` | Saved with warnings |
| `--crosshook-autosave-error-bg` / `--crosshook-autosave-error-border`     | Save failed         |

---

## Command palette overlay tokens

Used only in `palette.css` for the overlay surface. Do not use elsewhere — the palette
intentionally uses a deeper dark than the standard `--crosshook-color-bg`.

| Token                                | Value                       | Usage                     |
| ------------------------------------ | --------------------------- | ------------------------- |
| `--crosshook-palette-border-on-dark` | `rgba(255, 255, 255, 0.08)` | Palette surface border    |
| `--crosshook-palette-bg-dark-98`     | `rgba(13, 19, 34, 0.98)`    | Main palette backdrop     |
| `--crosshook-palette-bg-dark-90`     | `rgba(13, 19, 34, 0.9)`     | Palette row hover surface |

---

## Controller-mode overrides

`variables.css` defines a `:root[data-crosshook-controller-mode='true']` block that
overrides layout and spacing tokens for gamepad/Steam Deck controller mode. These apply
automatically when `useGamepadNav` detects a gamepad and sets the attribute on `<html>`.

Overridden tokens in controller mode include touch-target sizes, padding, and subtab
heights — all increased for D-Pad navigation comfort. Never reference these overrides
directly in component CSS; they apply globally via the attribute selector.

---

## Responsive @media overrides

`variables.css` contains three breakpoint-specific override blocks. These adjust spacing
and layout tokens automatically — no JS involvement.

| Block                        | Overrides                                          | Purpose                            |
| ---------------------------- | -------------------------------------------------- | ---------------------------------- |
| `@media (max-width: 1360px)` | `--crosshook-page-padding`, `--crosshook-grid-gap` | Laptop/narrow tightening           |
| `@media (max-width: 900px)`  | `--crosshook-page-padding`, `--crosshook-grid-gap` | Compact tightening                 |
| `@media (max-height: 820px)` | Touch targets, padding, launch panel tokens        | Short-viewport (Steam Deck native) |

---

## High-contrast theme token overrides

When `data-crosshook-theme='high-contrast'` is set on `<html>` (by
`useHighContrastTheme`), the following tokens are overridden. Components that use these
tokens automatically get high-contrast values without any conditional CSS.

Key overrides:

- Accent pair swaps from steel-blue to amber: `--crosshook-color-accent → #facc15`,
  `--crosshook-color-accent-strong → #f97316`
- Background and surface tokens shift to near-black for maximum contrast
- Border tokens increase opacity for higher contrast
- Status tokens shift to saturated values for unambiguous state communication

See `variables.css` `:root[data-crosshook-theme='high-contrast']` block for the complete
list. If adding new tokens that should be high-contrast-aware, add an override in that
block.
