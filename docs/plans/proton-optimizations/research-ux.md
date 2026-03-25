## Executive Summary

CrossHook should treat launch optimizations as a dedicated, profile-scoped card that lives with launch behavior, not as a free-form launch-string editor. The strongest pattern is an effect-first UI: human-readable checkboxes grouped by intent, with env vars and wrapper commands shown only as supporting details. That keeps the feature approachable for users who only want a few common tweaks, while still accommodating advanced Proton users.

The design should use semantic grouping (`fieldset`/`legend` or an equivalent grouped card), progressive disclosure for advanced and experimental options, and a visible launch preview so users can see how their toggles become `env vars -> wrappers -> %command% -> args` as described in the CachyOS gaming guide ([CachyOS gaming guide](https://wiki.cachyos.org/configuration/gaming/)). For accessibility and small screens, keep labels short, keep helper text concise, and ensure the section reflows into a single stacked column when the window narrows ([W3C labeling controls](https://www.w3.org/WAI/tutorials/forms/labels/), [W3C grouping controls](https://www.w3.org/WAI/tutorials/forms/grouping/), [Microsoft responsive content](https://learn.microsoft.com/en-us/style-guide/responsive-content/)).

### Core User Workflows

The common workflow is: open a Proton-backed profile, scan the launch card, enable one or two familiar optimizations such as `Disable Steam Input`, `Show frame overlay`, or `Enable HDR`, and launch without ever editing a raw command string. The UI should immediately show a summary like "3 optimizations enabled" so users can confirm the profile at a glance.

The advanced workflow is: expand a compact "Advanced" disclosure to reach experimental or hardware-specific options such as `Enable Wayland mode`, `Use NTSync`, `Enable NVIDIA libraries`, or `Steam Deck compatibility mode`. This keeps the default view shallow while still allowing power users to layer in options that are sometimes game-specific or driver-specific ([MDN details](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/details), [W3C accordion pattern](https://www.w3.org/WAI/ARIA/apg/patterns/accordion/)).

The error-recovery workflow is: a toggle is unavailable, greyed out, or annotated because the current launch method is native Linux, the GPU vendor does not match, or the selected Proton/runtime combination cannot use the option. The user should see why it is unavailable in plain language and be able to leave the profile unchanged rather than hitting a dead end.

For Steam Deck-sized layouts and other compact windows, the workflow should collapse into one vertical stack: LaunchPanel first, optimization card next, launcher export after that. Microsoft recommends designing for small viewports, short sections, and content that reflows from right-hand columns to stacked content as space shrinks ([Microsoft responsive content](https://learn.microsoft.com/en-us/style-guide/responsive-content/), [Microsoft forms](https://learn.microsoft.com/en-us/windows/apps/design/controls/forms?view=azurermps-1.7.0)).

## UI and Interaction Patterns

Use a top-level card titled `Launch Optimizations` with a short one-line description and a status summary row. That summary is important: it reduces the need to open the section just to understand what is active, and it gives the user a cheap way to verify that autosave worked.

Inside the card, group checkboxes by intent rather than by env var family. A practical first pass is `Input & Controller`, `Graphics & HDR`, `Performance & Overlay`, and `Compatibility Workarounds`. This matches how users think about troubleshooting games better than a list of variables would. W3C recommends grouping related controls with `fieldset` and `legend` so the relationship is clear both visually and programmatically ([W3C grouping controls](https://www.w3.org/WAI/tutorials/forms/grouping/)).

Each control should have a human-friendly visible label, with the env var or wrapper shown as secondary detail only. For example, `Disable Steam Input` can have helper text that says `PROTON_NO_STEAMINPUT=1`; `Show MangoHud overlay` can say `mangohud %command%`; `Use GameMode` can say `gamemoderun %command%`; and `Steam Deck compatibility mode` can say `SteamDeck=1` with a warning that it is a community workaround, not a universal fix. Visible labels should match the accessible name so speech-input and screen-reader users hear the same words they see ([W3C labels](https://www.w3.org/WAI/tutorials/forms/labels/), [W3C label in name](https://www.w3.org/WAI/WCAG22/Understanding/label-in-name.html)).

Wrappers need special handling because ordering matters. The UI should not become a raw command editor, and it should not let users accidentally create invalid combinations like multiple `%command%` markers. Instead, treat wrappers as a bounded sub-group with a deterministic preview of the final launch prefix. The CachyOS guide is explicit that launch options are ordered as env vars, wrappers, `%command%`, and arguments, and that multiple `%command%` tokens are a mistake ([CachyOS gaming guide](https://wiki.cachyos.org/configuration/gaming/)).

Progressive disclosure should be used for anything experimental or noisy. The default view should show the common toggles directly, while a compact `Advanced` disclosure can hold options like `Enable Wayland mode`, `Use NTSync`, `Enable NVIDIA NVML`, `Enable media converter`, or `Steam Deck compatibility mode`. `<details>`/`<summary>` is a good fit for this because it is compact, keyboard-friendly, and exposes open/closed state without custom scripting ([MDN details](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/details)).

## Accessibility Considerations

Use native checkboxes and native labels wherever possible. Do not rely on placeholders or helper text as the only label. Related controls should be grouped semantically, not just visually, so screen readers can announce the section context when users move through the toggles ([W3C labels](https://www.w3.org/WAI/tutorials/forms/labels/), [W3C grouping controls](https://www.w3.org/WAI/tutorials/forms/grouping/)).

Keep the visible label text short and exact. The label should describe the effect, not the implementation. For example, `Disable Steam Input` is better than `PROTON_NO_STEAMINPUT` because the latter is a backend detail, not a user goal. W3C guidance on label-in-name is important here: the visible label and the accessible name should match or closely align ([W3C label in name](https://www.w3.org/WAI/WCAG22/Understanding/label-in-name.html)).

Support keyboard-only use and controller use by preserving a linear tab order and keeping hit targets large enough for touch-like input. The app is used on Steam Deck-style screens, so the section should avoid dense multi-column checkbox grids until the window is wide enough to support them cleanly. Microsoft recommends keeping content short, minimizing columns, and designing for small screen flow first ([Microsoft responsive content](https://learn.microsoft.com/en-us/style-guide/responsive-content/), [Microsoft forms](https://learn.microsoft.com/en-us/windows/apps/design/controls/forms?view=azurermps-1.7.0)).

Use `aria-describedby` for dependency hints and warnings, not for the primary label. That keeps the label concise while still exposing important context such as `Requires HDR-capable output`, `Experimental`, or `Only applies to Proton-backed launches`. If a disclosure is used for advanced settings, its summary must have a non-empty accessible name and its expanded state should be clear to assistive technologies ([W3C instructions](https://www.w3.org/WAI/tutorials/forms/instructions/), [W3C APG accordion](https://www.w3.org/WAI/ARIA/apg/patterns/accordion/)).

### Feedback and State Design

Autosave needs visible feedback because the user is no longer pressing a Save button for this section. Show a small inline status such as `Saving...`, `Saved automatically`, or `Failed to save` near the optimization card summary. This should be polite and low-noise, but still explicit enough that the user trusts the state.

Show state at the control level as well: disabled, unavailable, recommended, and experimental should each have a distinct visual treatment. A toggle that cannot apply to native launches should be disabled with a short explanation. A toggle that is hardware-specific should stay enabled only when it is meaningful, and otherwise it should read as unsupported instead of silently doing nothing.

Preview matters more than decoration. Whenever a toggle changes the launch composition, update a one-line launch preview in place so users can see the effect of selecting MangoHud, GameMode, or a Proton env var without opening another screen. This is especially important for wrapper order, because the difference between `mangohud game-performance %command%` and `game-performance mangohud %command%` is not obvious to most users.

For experimental or community-only options like `SteamDeck=1`, use a warning badge and a short explanation that frames it as a compatibility workaround. That avoids overpromising and makes it clear that the feature is there because some games need it, not because it is a general optimization. The user should be able to see the risk before they enable it.

## UX Risks

The biggest risk is overload. If every available env var gets a checkbox in the default view, the feature will feel like a troubleshooting wiki page instead of a launcher control. The likely result is abandonment or random toggling without understanding. The fix is to keep the default surface small and move niche items into `Advanced`.

Another risk is false confidence. Several of these toggles only help in specific situations: `PROTON_ENABLE_HDR` depends on the display path, `PROTON_NVIDIA_LIBS` only matters for NVIDIA scenarios, and `SteamDeck=1` is a workaround for a narrow class of games. If the UI makes them look universally beneficial, users will treat them like generic performance boosts and may blame CrossHook when a game regresses.

Wrapper confusion is a third risk. `MangoHud`, `GameMode`, and `game-performance` are not interchangeable, and they are not raw env vars. If the UI presents them as equivalent checkboxes with no preview, users may create an invalid or redundant launch string. The interface should therefore show the generated prefix and avoid letting users free-type into the command structure.

The final risk is layout density on small windows. The current CrossHook screen already has a strong two-column composition, but the optimization section can easily become too tall if it tries to show every option at once. The safer pattern is a compact default card, a short summary line, and an expandable advanced section that preserves vertical space on Steam Deck-sized screens and smaller desktop windows.
