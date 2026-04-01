# External Research: UI Libraries and Patterns for CrossHook UI Enhancements

**Date**: 2026-03-31
**Task**: Research external APIs, libraries, and integration patterns for the Profiles page Advanced section declutter.

---

## Executive Summary

CrossHook already has `@radix-ui/react-tabs` v1.1.13 in its `package.json`. The project uses **no CSS framework** (no Tailwind, no MUI) — it uses a custom CSS variable system with BEM-like `crosshook-*` classes and CSS variables in `src/styles/variables.css`. This is important: any library must work with plain CSS/CSS variables, not Tailwind-only APIs.

The primary UI layout problem is that the Profiles page Advanced section collapses most of the form. The codebase already has:

- `@radix-ui/react-tabs` (already installed, partially in use)
- A custom `CollapsibleSection` component (`src/components/ui/CollapsibleSection`) using native `<details>/<summary>`
- `react-resizable-panels` for resizable splits

**Critical update (from business-analyzer)**: The sub-tab CSS classes are already defined in `theme.css` but unused. `.crosshook-subtab-row`, `.crosshook-subtab`, and `.crosshook-subtab--active` exist with pill styling, accent gradient active state, and responsive overrides. The `--crosshook-subtab-min-height` and `--crosshook-subtab-padding-inline` variables are also pre-defined. Implementation cost is lower than initially estimated — the CSS layer is already ready.

**Recommendation summary**: The project already has all the primitives needed. The best path forward uses **zero new dependencies** — extend the existing Radix `Tabs` install with the pre-built `.crosshook-subtab*` CSS classes. If additional headless primitives are needed, `@radix-ui/react-accordion` is the most natural add since it is same vendor, same tree-shakable pattern.

**Confidence**: High (based on verified `package.json`, confirmed CSS in `theme.css`, plus Radix official docs)

---

## Current Technology Stack (Verified)

From `src/crosshook-native/package.json`:

| Dependency               | Version | Role                               |
| ------------------------ | ------- | ---------------------------------- |
| `@radix-ui/react-tabs`   | ^1.1.13 | Tab primitives (already installed) |
| `@radix-ui/react-select` | ^2.2.6  | Select primitives                  |
| `react-resizable-panels` | ^4.7.6  | Resizable panel splits             |
| `@tauri-apps/api`        | ^2.0.0  | Tauri IPC                          |
| React                    | ^18.3.1 | Frontend framework                 |
| Vite                     | ^8.0.2  | Bundler                            |
| TypeScript               | ^5.6.3  | Type system                        |

**No Tailwind CSS, no MUI, no design system framework** — the project uses a handcrafted CSS variable system with `crosshook-*` CSS custom properties defined in `src/styles/variables.css`.

---

## Primary APIs

> For this UI-focused feature, "APIs" refers to UI component library APIs rather than external web services.

## Primary UI Libraries

### 1. Radix UI Primitives (ALREADY INSTALLED)

- **Docs**: <https://www.radix-ui.com/primitives>
- **Tabs docs**: <https://www.radix-ui.com/primitives/docs/components/tabs>
- **Accordion docs**: <https://www.radix-ui.com/primitives/docs/components/accordion>
- **Version already in use**: `@radix-ui/react-tabs` v1.1.13
- **Bundle size**: `@radix-ui/react-tabs` ~90kB minified (individual packages, tree-shaken per component)
- **Tauri compatibility**: Full — headless, no browser-specific APIs, pure DOM manipulation

**Tabs API**:

```typescript
import * as Tabs from '@radix-ui/react-tabs';

<Tabs.Root defaultValue="general" orientation="vertical">
  <Tabs.List aria-label="Profile sections">
    <Tabs.Trigger value="general">General</Tabs.Trigger>
    <Tabs.Trigger value="runtime">Runtime</Tabs.Trigger>
    <Tabs.Trigger value="advanced">Advanced</Tabs.Trigger>
  </Tabs.List>
  <Tabs.Content value="general">...</Tabs.Content>
  <Tabs.Content value="runtime">...</Tabs.Content>
  <Tabs.Content value="advanced">...</Tabs.Content>
</Tabs.Root>
```

**Keyboard navigation** (built-in):

- `Tab` / `Shift+Tab` — move between trigger list and panel
- `ArrowLeft` / `ArrowRight` (horizontal) or `ArrowUp` / `ArrowDown` (vertical) — navigate triggers
- `Home` / `End` — jump to first/last trigger

**Key props**:

- `orientation`: `"horizontal"` | `"vertical"` — vertical is ideal for a sidebar-style sub-nav
- `activationMode`: `"automatic"` (default, activates on arrow key) | `"manual"` (requires Enter/Space)
- `defaultValue` / `value` — uncontrolled or controlled mode

**Confidence**: High — official docs verified, already in `package.json`

---

### 2. Radix UI Accordion (NOT YET INSTALLED — natural add)

- **Install**: `npm install @radix-ui/react-accordion`
- **Docs**: <https://www.radix-ui.com/primitives/docs/components/accordion>
- **Bundle size**: ~90.2kB minified (v1.2.12, per bundlephobia)
- **Tauri compatibility**: Full

**Accordion API**:

```typescript
import * as Accordion from '@radix-ui/react-accordion';

// Single open item
<Accordion.Root type="single" defaultValue="general" collapsible>
  <Accordion.Item value="general">
    <Accordion.Header>
      <Accordion.Trigger>General</Accordion.Trigger>
    </Accordion.Header>
    <Accordion.Content>...</Accordion.Content>
  </Accordion.Item>
</Accordion.Root>

// Multiple open simultaneously
<Accordion.Root type="multiple">
  ...
</Accordion.Root>
```

**Key props**:

- `type`: `"single"` | `"multiple"` — whether one or many sections can be open at once
- `collapsible`: `boolean` — allows all items to be closed (only for `type="single"`)
- `defaultValue` / `value` — controlled or uncontrolled open state

**Keyboard navigation** (built-in):

- `Space` / `Enter` — expand/collapse focused trigger
- `ArrowDown` / `ArrowUp` — move between triggers
- `Home` / `End` — first/last trigger

**Advantage over current `<details>/<summary>`**: Radix Accordion is fully accessible (WAI-ARIA), animatable via CSS custom properties (`--radix-accordion-content-height`), and consistent with the existing Radix primitives already in use.

**Confidence**: High — official docs verified, same vendor pattern as existing deps

---

### 3. shadcn/ui (Copy-paste component approach — not a package)

- **Docs**: <https://ui.shadcn.com/>
- **Tabs**: <https://ui.shadcn.com/docs/components/radix/tabs>
- **Collapsible**: <https://ui.shadcn.com/docs/components/radix/collapsible>
- **Install model**: Components are copied into your codebase via `pnpm dlx shadcn@latest add tabs`

**Key distinction**: shadcn/ui is NOT a library you install as a dependency. It generates React component files that are added to your project. These components are thin wrappers around Radix UI primitives, styled with Tailwind CSS.

**Compatibility with CrossHook**: **Problematic**. shadcn/ui components assume Tailwind CSS. CrossHook does not use Tailwind. You would need to strip all Tailwind class names and replace with custom CSS, defeating the purpose of shadcn.

**Recommendation**: Skip. The underlying Radix primitives (already installed) are what shadcn uses. Go direct to Radix instead.

**Confidence**: High — docs verified, but Tailwind requirement confirmed as incompatible

---

### 4. Headless UI (by Tailwind Labs)

- **Docs**: <https://headlessui.com/react/tabs>
- **Install**: `npm install @headlessui/react`
- **Version**: Currently v2.x
- **Tauri compatibility**: Full

**Tabs API**:

```typescript
import { Tab, TabGroup, TabList, TabPanel, TabPanels } from '@headlessui/react';

<TabGroup vertical>
  <TabList>
    <Tab>General</Tab>
    <Tab>Runtime</Tab>
  </TabList>
  <TabPanels>
    <TabPanel>...</TabPanel>
    <TabPanel>...</TabPanel>
  </TabPanels>
</TabGroup>
```

**Key props**:

- `vertical`: boolean — enables up/down arrow navigation
- `manual`: boolean — require Enter/Space to activate instead of auto on arrow key
- `defaultIndex` / `selectedIndex` + `onChange` — controlled mode

**Keyboard navigation**: Arrow keys (direction-aware), Home/End, Enter/Space in manual mode

**Styling**: Uses `data-selected`, `data-hover`, `data-focus`, `data-active` attributes — compatible with plain CSS selectors (no Tailwind required).

**Recommendation**: Valid alternative to Radix Tabs, but adds a new dependency when Radix is already installed. Radix is preferable for consistency.

**Confidence**: High — official docs verified

---

### 5. Ark UI (by Chakra team, powered by Zag.js state machines)

- **Docs**: <https://ark-ui.com/>
- **NPM**: `@ark-ui/react`
- **Components**: 45+ including Tabs, Accordion, Collapsible, Select, Dialog
- **Tauri compatibility**: Full — headless, no browser-specific APIs

**Strengths**:

- State machine-based (Zag.js) — very predictable behavior
- Works with any CSS approach (no Tailwind required)
- WAI-ARIA compliant, keyboard navigation built-in

**Weaknesses**:

- Would add `@ark-ui/react` + `@zag-js/*` as new dependencies
- CrossHook already has Radix which covers the same surface area
- Larger dependency footprint than individual `@radix-ui/react-*` packages

**Recommendation**: Skip for this project. Overkill when Radix is already installed.

**Confidence**: Medium — home page and npm verified, but not in current stack

---

## Tab/Navigation Libraries (Evaluated)

| Library                     | Install                           | Size                   | Tailwind Required | Already Installed |
| --------------------------- | --------------------------------- | ---------------------- | ----------------- | ----------------- |
| `@radix-ui/react-tabs`      | Yes                               | ~90kB                  | No                | **YES**           |
| `@radix-ui/react-accordion` | `npm i @radix-ui/react-accordion` | ~90kB                  | No                | No                |
| `@headlessui/react`         | `npm i @headlessui/react`         | ~22kB                  | No                | No                |
| `ark-ui`                    | `npm i @ark-ui/react`             | Large (+ zag)          | No                | No                |
| `shadcn/ui tabs`            | Copy-paste                        | 0 (but needs Tailwind) | **YES**           | No                |

**Winner**: `@radix-ui/react-tabs` — already installed, zero new dependency cost, consistent API with existing code.

---

## Layout Pattern Libraries

### react-resizable-panels (ALREADY INSTALLED)

- **Version**: `^4.7.6`
- **Already in use** for the main content area split
- Not directly useful for sub-tab navigation, but relevant for any future panel splits within the Profiles page

---

## Integration Patterns

### Pattern 1: Vertical Sub-Tabs (Recommended for "sub-tabs within pages")

Use `@radix-ui/react-tabs` with `orientation="vertical"` to replace the single collapsed Advanced `<details>` block with a persistent sidebar + panel layout.

```
[Profile/Launcher Export]  [  General  |  Runtime  |  Advanced  ]
                            ┌──────────┬─────────────────────────┐
                            │ General  │                         │
                            │ Runtime  │  <TabPanel content>     │
                            │ Advanced │                         │
                            └──────────┴─────────────────────────┘
```

**CSS approach** (no new deps, uses existing design tokens):

```css
.crosshook-profile-subtabs {
  display: grid;
  grid-template-columns: 160px 1fr;
  gap: 0;
  height: 100%;
}

.crosshook-profile-subtabs__list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 4px;
  border-right: 1px solid var(--crosshook-color-border);
}

.crosshook-profile-subtabs__trigger {
  padding: 10px 12px;
  border-radius: var(--crosshook-radius-sm);
  color: var(--crosshook-color-text-muted);
  text-align: left;
  background: transparent;
  border: none;
  cursor: pointer;
  transition:
    background var(--crosshook-transition-fast),
    color var(--crosshook-transition-fast);
}

.crosshook-profile-subtabs__trigger[data-state='active'] {
  background: var(--crosshook-color-accent-soft);
  color: var(--crosshook-color-accent-strong);
}
```

**Confidence**: High

---

### Pattern 2: Section Cards with Clear Boundaries

Replace the single collapsed Advanced block with promoted sections in visually distinct cards. Uses the existing `crosshook-panel` class pattern already in the codebase.

```
┌─────────────────────────────────────┐  ┌─────────────────────────────────────┐
│  General / Core                     │  │  Launch & Runtime                   │
│  [profile name, game path, etc.]    │  │  [proton, env vars, working dir]    │
└─────────────────────────────────────┘  └─────────────────────────────────────┘
┌─────────────────────────────────────┐  ┌─────────────────────────────────────┐
│  ProtonDB                           │  │  Export                             │
│  [lookup card]                      │  │  [launcher/community export]        │
└─────────────────────────────────────┘  └─────────────────────────────────────┘
```

**Zero new dependencies** — uses existing `crosshook-panel`, CSS grid, and design tokens.

**Confidence**: High

---

### Pattern 3: Progressive Disclosure via Radix Accordion

Replace the native `<details>/<summary>` collapsible with `@radix-ui/react-accordion` for animated, accessible section disclosure.

**Advantage over current `<details>`**: Radix Accordion supports `type="multiple"` (multiple open at once), CSS animation on `--radix-accordion-content-height`, and full WAI-ARIA compliance.

**Confidence**: High

---

### Pattern 4: Promoted Sections (No Collapse)

Move the most-used subsections (e.g., ProtonDB lookup, Launch Options) out of the collapsed Advanced area and make them always visible at the top level, retaining only truly advanced/rarely-used settings in a collapse.

**Zero new dependencies** — pure layout restructuring.

**Confidence**: High

---

## Constraints and Gotchas

### CrossHook-Specific

1. **No Tailwind**: The project uses a custom CSS variable system. shadcn/ui and any Tailwind-first library requires manual de-Tailwindification — avoid.
2. **@radix-ui/react-tabs already in package.json** — extending its use is zero cost. The sub-tab CSS classes (`.crosshook-subtab-row`, `.crosshook-subtab`, `.crosshook-subtab--active`) are already defined in `theme.css` and ready to use.
3. **Nested Tabs.Root architecture** (from business-analyzer): The app's outer `Tabs.Root` uses `orientation="vertical"` for page-level routing. Any within-page sub-tabs must be a **nested `Tabs.Root`** with `orientation="horizontal"`. Radix supports nested roots with independent value scopes — the inner root must not inherit the outer orientation. Do not attempt to reuse the outer root for sub-navigation.
4. **Composition constraint** (from business-analyzer, `research-business.md` §Implementation Constraints §1): Sub-tabs must be composed at the **`ProfilesPage` level**, not inside `ProfileFormSections`, due to `InstallPage` modal reuse of `ProfileFormSections`. The form sections component is shared and cannot own the tab state.
5. **CSS variables vs. inline styles**: The codebase uses both `crosshook-*` CSS classes and inline `style` objects (e.g., `optionalSectionStyle` in `ProfileFormSections.tsx:60`). New UI should prefer CSS classes with design token variables per CLAUDE.md convention.
6. **`react-resizable-panels` v4.7.6** is present. If a sub-tab layout needs resizable panes, this can be leveraged.

### Tauri / WebKitGTK Linux

1. **WebKitGTK 4.1 (Linux)**: Full CSS Grid, Flexbox, CSS custom properties, CSS transitions — all supported. No known issues with tab or accordion components.
2. **No Node.js integration**: All Tauri IPC must go through `invoke()` — irrelevant to pure UI layout changes.
3. **Bundle size**: Tauri already ships system WebView, so JS bundle size matters less than in Electron. Radix individual packages (~90kB each minified) are acceptable; a full MUI install (~500kB+) would be excessive.
4. **System WebView inconsistencies**: Minor rendering differences between WebKitGTK (Linux), WebView2 (Windows), and WKWebView (macOS) are possible but CSS grid/flexbox layouts are well-supported across all three.

### Progressive Disclosure UX

Per Nielsen Norman Group research:

- Progressive disclosure improves learnability, efficiency, and error rate
- Best practice: define via user research (card sorting, task analysis) which items are primary vs. advanced
- Too many collapsed sections defeat the purpose — promote the most-used 20% to always-visible

---

## Code Examples

### Sub-tabs with existing Radix install (nested root, horizontal orientation)

The outer app `Tabs.Root` uses `orientation="vertical"` for page routing. Sub-tabs within ProfilesPage must be a **separate nested `Tabs.Root`** — composed at `ProfilesPage` level, not inside `ProfileFormSections`.

```tsx
// Uses @radix-ui/react-tabs already in package.json
// Uses .crosshook-subtab-row / .crosshook-subtab / .crosshook-subtab--active already in theme.css
import * as Tabs from '@radix-ui/react-tabs';

// Composed at ProfilesPage level — NOT inside ProfileFormSections
export function ProfilesPage() {
  return (
    // ... outer page structure ...
    <Tabs.Root defaultValue="general" orientation="horizontal">
      {' '}
      {/* nested root — independent scope */}
      <Tabs.List className="crosshook-subtab-row" aria-label="Profile sections">
        <Tabs.Trigger
          className="crosshook-subtab" // .crosshook-subtab--active applied via data-state
          value="general"
        >
          General
        </Tabs.Trigger>
        <Tabs.Trigger className="crosshook-subtab" value="runtime">
          Runtime
        </Tabs.Trigger>
        <Tabs.Trigger className="crosshook-subtab" value="advanced">
          Advanced
        </Tabs.Trigger>
      </Tabs.List>
      <Tabs.Content value="general">{/* Game path, profile name, ProtonDB lookup */}</Tabs.Content>
      <Tabs.Content value="runtime">{/* Proton version, env vars, working directory */}</Tabs.Content>
      <Tabs.Content value="advanced">
        {/* Steam launch options, steam client path, less common settings */}
      </Tabs.Content>
    </Tabs.Root>
  );
}
```

**Note**: The `.crosshook-subtab--active` class must be applied conditionally based on Radix's `data-state="active"` attribute, or via a CSS selector `[data-state="active"]`. The existing theme.css classes use a manual `--active` suffix; a CSS attribute selector in the stylesheet is the cleanest bridge:

```css
/* In theme.css or a new profile-subtabs.css */
.crosshook-subtab[data-state='active'] {
  border-color: rgba(0, 120, 212, 0.45);
  background: linear-gradient(135deg, var(--crosshook-color-accent) 0%, var(--crosshook-color-accent-strong) 100%);
  color: #fff;
}
```

### Radix Accordion (animated, replaces native `<details>`)

```tsx
import * as Accordion from '@radix-ui/react-accordion';

// CSS animation: target --radix-accordion-content-height
// .crosshook-accordion-content { overflow: hidden; }
// .crosshook-accordion-content[data-state="open"] { animation: slideDown 220ms ease; }
// .crosshook-accordion-content[data-state="closed"] { animation: slideUp 220ms ease; }

export function ProfileAccordion() {
  return (
    <Accordion.Root type="multiple" className="crosshook-accordion">
      <Accordion.Item value="launch" className="crosshook-accordion__item">
        <Accordion.Header>
          <Accordion.Trigger className="crosshook-accordion__trigger">Launch Options</Accordion.Trigger>
        </Accordion.Header>
        <Accordion.Content className="crosshook-accordion__content">{/* launch options */}</Accordion.Content>
      </Accordion.Item>
      <Accordion.Item value="env" className="crosshook-accordion__item">
        <Accordion.Header>
          <Accordion.Trigger className="crosshook-accordion__trigger">Environment Variables</Accordion.Trigger>
        </Accordion.Header>
        <Accordion.Content className="crosshook-accordion__content">{/* env vars */}</Accordion.Content>
      </Accordion.Item>
    </Accordion.Root>
  );
}
```

---

## Open Questions

1. **Which sections are actually in the collapsed Advanced area?** Full enumeration of `ProfileFormSections.tsx` sections needed to decide what to promote vs. keep collapsed.
2. **Does the existing Tabs install (`@radix-ui/react-tabs`) render anywhere currently?** If yes, where — to understand whether extending it for sub-tabs introduces visual inconsistency.
3. **User research on frequency of use**: Which sections are accessed most by users? This determines what to promote vs. keep in advanced disclosure.
4. **Controller mode (`data-crosshook-controller-mode`)**: The CSS variables file has a controller mode override — sub-tab touch targets need to scale appropriately (`--crosshook-subtab-min-height` and `--crosshook-subtab-padding-inline` are already defined, suggesting sub-tabs were considered).
5. **Animation tolerance**: Is CSS transition animation on accordion expand/collapse acceptable in the WebKitGTK environment? (Generally yes, but worth noting if users report jank.)

---

## Sources

- [Radix UI Tabs primitives docs](https://www.radix-ui.com/primitives/docs/components/tabs)
- [Radix UI Accordion primitives docs](https://www.radix-ui.com/primitives/docs/components/accordion)
- [shadcn/ui Tabs](https://ui.shadcn.com/docs/components/radix/tabs)
- [shadcn/ui Collapsible](https://ui.shadcn.com/docs/components/radix/collapsible)
- [Headless UI Tabs](https://headlessui.com/react/tabs)
- [Ark UI home](https://ark-ui.com/)
- [Ark UI npm](https://www.npmjs.com/package/@ark-ui/react)
- [CrabNebula — Best UI Libraries for Tauri](https://crabnebula.dev/blog/the-best-ui-libraries-for-cross-platform-apps-with-tauri/)
- [Tauri v2 Bundle Size Optimization](https://www.oflight.co.jp/en/columns/tauri-v2-performance-bundle-size)
- [Tauri v2 Stable Release](https://v2.tauri.app/blog/tauri-20/)
- [Nielsen Norman Group — Progressive Disclosure](https://www.nngroup.com/articles/progressive-disclosure/)
- [LogRocket — Progressive Disclosure UX types and use cases](https://blog.logrocket.com/ux-design/progressive-disclosure-ux-types-use-cases/)
- [15 Best React UI Libraries for 2026 — Builder.io](https://www.builder.io/blog/react-component-libraries-2026)
- [@radix-ui/react-accordion on npm](https://www.npmjs.com/package/@radix-ui/react-accordion)
- [@radix-ui/react-tabs on npm](https://www.npmjs.com/package/@radix-ui/react-tabs)

---

## Search Queries Executed

1. `Radix UI tabs accordion components React desktop app Tauri 2026`
2. `shadcn/ui tabs collapsible components progressive disclosure settings page React TypeScript`
3. `Headless UI React tab component keyboard navigation accessibility desktop`
4. `React settings page layout patterns sidebar navigation sub-tabs desktop 2024 2025`
5. `ark-ui zagjs react component library headless tabs accordion npm 2024 2025`
6. `shadcn/ui bundle size tree-shaking npm package size 2024 2025`
7. `Radix UI npm package size @radix-ui/react-tabs @radix-ui/react-accordion bundle 2024`
8. `Tauri v2 React UI library compatibility bundle size WebView limitations 2024 2025`
9. `progressive disclosure UX pattern settings page advanced options design system React`
10. `React settings page visual design examples cards sections dividers 2024`
11. `React vertical tab sidebar settings layout no external dependency custom CSS 2024`
12. `Tauri v2 WebKitGTK Linux CSS grid flexbox sub-tabs sidebar layout performance 2024`

---

## Uncertainties and Gaps

- **Exact bundle sizes** for individual Radix packages could not be confirmed from Bundlephobia (site returned 502 during research). The ~90kB figure comes from npm page metadata.
- **shadcn/ui** "copy-paste" model means the "library" has no npm bundle size — components live in your repo. This is confirmed in docs but worth noting.
- **Ark UI** bundle size with Zag.js state machines was not confirmed numerically.
- **Headless UI v2 full changelog** was not reviewed — verify no breaking changes from v1 if this path is chosen.
