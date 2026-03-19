# Recommendations: dotnet-migrate

## Recommendation Summary

The strongest migration path is a narrow, behavior-preserving move to `net9.0-windows` with WinForms kept in place and WINE/Proton validation elevated to a hard phase gate. The plan should modernize the build, clean up lifecycle/resource issues, and convert the existing interop layer to `[LibraryImport]`. It should not combine that work with a UI rewrite, a config-format rewrite, or a new multi-project architecture.

## Why This Scope Makes Sense

- It aligns with the current codebase shape: one app project, one large form, three manager classes.
- It minimizes moving parts before the WINE/Proton viability check.
- It preserves existing profiles and settings.
- It targets the highest-value improvements first:
  - modern project tooling
  - self-contained publish path
  - explicit cleanup of lifecycle/resource bugs
  - interop modernization

## What To Do In This Migration

### Required

- convert to SDK-style project
- map assembly metadata intentionally
- update contributor docs
- prove the published build under WINE/Proton
- fix duplicate subscriptions and `FormClosing` recursion
- add explicit cleanup for `ProcessManager` and `InjectionManager`
- extract only low-risk persistence and CLI parsing code
- convert interop declarations to `[LibraryImport]`

### Explicit Release Decisions

- choose `win-x64`, `win-x86`, or dual artifacts
- decide whether `-dllinject` is implemented or removed from docs for the migration release
- decide whether `ComVisible(false)` and assembly `Guid` remain in a slimmed `AssemblyInfo.cs`

## What To Defer

- Avalonia
- `CrossHookEngine.Core` split
- JSON/TOML migration
- `LaunchOrchestrator`
- broad nullable/style modernization
- interop deduplication into shared files unless it still looks cheap after regression proof

## Framework Note

As of March 18, 2026, both `.NET 8` and `.NET 9` remain supported through November 2026, and `.NET 10` is already available as the current LTS train. That means lifecycle alone is not the main planning variable here; the real risk is runtime behavior under WINE/Proton. This package therefore keeps the researched `net9.0-windows` target and avoids reopening the migration around a second framework jump. If the team wants a longer-lived target, retargeting to `.NET 10` should be treated as a follow-up after the base migration is proven.
