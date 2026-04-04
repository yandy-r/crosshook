# Game details modal — follow-up ideas (non-blocking)

1. **Single profile fetch**: Avoid double `profile_load` when the selected profile already matches and is loaded in context (measure first).
2. **Offline readiness provider**: Share one `useOfflineReadiness` instance app-wide to avoid parallel cache hydration on Library vs Profiles.
3. **List view**: If the library gains a non-card list layout, replicate the same body vs action hit targets there.
4. **Deep link / URL**: Optional future route or query to open details for a named profile (out of v1 scope).
5. **Richer ProtonDB**: Surface one-line recommendation summary from `recommendation_groups` when present (keep plain text only).
