# Game details modal — manual verification

Run these in a graphical Tauri build (`./scripts/dev-native.sh` or a release AppImage). Record results in `verification-results.md`.

## Library card interactions

- [ ] Clicking the card artwork / open area opens the details modal and selects that profile (sidebar / launch context matches).
- [ ] `Launch`, `Favorite`, and `Edit` in the card footer behave as before; they do not open the modal.
- [ ] Keyboard: focus the details hitbox with Tab; Enter/Space opens the modal.

## Modal chrome

- [ ] `Escape` closes the modal and focus returns to a sensible control.
- [ ] Clicking the dimmed backdrop closes the modal.
- [ ] Close button closes the modal and is reachable with Tab order.
- [ ] With a controller / gamepad, global back handling still closes the modal when focus is inside (uses `data-crosshook-modal-close` / focus root).

## Content and degraded states

- [ ] Profile with no Steam App ID: modal opens; metadata and ProtonDB sections show unavailable copy; paths still show from loaded profile when load succeeds.
- [ ] Offline or failed remote metadata: section shows unavailable or cached/stale messaging without crashing the modal.
- [ ] Rapidly open details for profile A then B: loaded paths and title match profile B (no stale overwrite).

## Scroll and narrow viewport

- [ ] At ~1280×800 and smaller, only the modal body scrolls; header/footer stay visible.
- [ ] Mouse wheel / trackpad scroll feels correct (WebKitGTK uses `.crosshook-modal__body` in `useScrollEnhance`).

## Quick actions

- [ ] `Launch` closes the modal then runs the existing launch navigation flow.
- [ ] `Edit profile` closes the modal then navigates to Profiles.
- [ ] `Favorite` toggles favorite without closing the modal; library card favorite state updates.
