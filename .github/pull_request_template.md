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

- **Platform**: <!-- Steam Deck / Linux distro / macOS -->
- **Proton / WINE Version**: <!-- e.g., Proton 9.0-1 -->
- **Game / Trainer** (if applicable): <!-- e.g., Elden Ring + FLiNG v1.2.3 -->

### Checklist

- [ ] `msbuild src/ChooChooEngine.sln /p:Configuration=Debug` builds without errors
- [ ] `msbuild src/ChooChooEngine.sln /p:Configuration=Release` builds without errors
- [ ] Tested under Proton/WINE on target platform
- [ ] **If touching Injection/**: Verified DLL injection works with at least one trainer
- [ ] **If touching Memory/**: Verified memory read/write operations
- [ ] **If touching Core/ProcessManager**: Verified process launch, attach, and lifecycle
- [ ] **If touching Forms/ or UI/**: Verified UI renders correctly under WINE

## Reviewer Notes

<!-- Anything reviewers should know: risks, areas needing extra scrutiny, migration notes -->
