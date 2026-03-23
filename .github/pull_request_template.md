## Summary

<!-- Brief description of what this PR does and why -->

Closes #<!-- issue number -->

## Changes

-

## Type of Change

- [ ] Bug fix (non-breaking change that fixes an issue)
- [ ] New feature (non-breaking change that adds functionality)
- [ ] Refactor (no functional changes)
- [ ] Breaking change (fix or feature that would cause existing functionality to change)
- [ ] Documentation
- [ ] Build / CI
- [ ] Compatibility (new game/trainer/platform support)

## Testing

### Environment

- **Platform**: <!-- Linux distro / Steam Deck -->
- **Proton Version** (if applicable): <!-- e.g., Proton 9.0-1 -->
- **Game / Trainer** (if applicable): <!-- e.g., Elden Ring + FLiNG v1.2.3 -->

### Checklist

- [ ] `./scripts/build-native.sh --binary-only` builds without errors
- [ ] `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` passes
- [ ] `./scripts/build-native.sh` produces a valid AppImage (if touching build/packaging)
- [ ] Tested on target platform (Linux desktop or Steam Deck)
- [ ] **If touching crates/crosshook-core/src/launch/**: Verified game and trainer launch works
- [ ] **If touching crates/crosshook-core/src/steam/**: Verified Steam auto-populate and Proton discovery
- [ ] **If touching crates/crosshook-core/src/profile/**: Verified profile save/load/import
- [ ] **If touching src/components/ or src/hooks/**: Verified UI renders correctly
- [ ] **If touching runtime-helpers/**: Verified shell scripts work under Proton

## Reviewer Notes

<!-- Anything reviewers should know: risks, areas needing extra scrutiny, migration notes -->
