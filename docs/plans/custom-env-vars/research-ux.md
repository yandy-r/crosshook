# Custom Env Vars: UX Research

## UX Goals

- Make custom env vars easy to author and review.
- Prevent invalid/dangerous entries early.
- Make precedence behavior obvious before launch.

## Editor Pattern

Recommended UI in `ProfileFormSections` launch/runtime area:

- Section title: `Custom Environment Variables`
- Table/list rows with:
  - `Key` input
  - `Value` input
  - remove action
- Add-row button and clear empty-state copy.

## Interaction Guidelines

- Inline validation on blur/change for `key`.
- Immediate duplicate-key detection inside the table.
- Save remains available, but profile validation/preview launch blocks invalid rows with actionable errors.
- Keyboard support:
  - Tab/Shift+Tab between fields
  - Enter to add/confirm row
  - Delete/remove via focused button

## Precedence Communication

Add short help copy near editor:

`Custom variables override built-in launch optimization variables when keys conflict.`

In preview modal:

- Show source badges including `Profile custom`.
- If a key is overridden, show the winning value only (optional expandable "overridden" detail).

## Empty and Error States

- Empty state: `No custom variables configured for this profile.`
- Invalid key: `Use a non-empty key without "=" characters.`
- Duplicate key: `This key already exists in the table.`
- Reserved key block (if configured): `This key is managed by CrossHook runtime and cannot be overridden.`

## Accessibility Notes

- Inputs with `aria-invalid` when failing validation.
- Errors connected via `aria-describedby`.
- Maintain visible focus styles and non-color-only error indicators.

## Recommended UX Scope (Feature Complete)

- Add/edit/remove rows
- Inline validation messaging
- Precedence helper copy
- Preview source visibility for custom vars
- Consistent behavior across `proton_run`, `steam_applaunch`, `native`

## References

- [WAI-ARIA grid pattern](https://www.w3.org/WAI/ARIA/apg/patterns/grid/)
- [WCAG focus visible](https://www.w3.org/WAI/WCAG22/Understanding/focus-visible)
- [WCAG error identification](https://www.w3.org/WAI/WCAG21/understanding/error-identification.html)
