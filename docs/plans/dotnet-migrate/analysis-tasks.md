# Task Structure Analysis: dotnet-migrate

## Executive Summary

The original task set was too optimistic about parallelism and too loose about scope. The corrected structure has four real dependency groups:

1. decision freeze
2. foundation plus WINE/Proton gate
3. `MainForm` cleanup and low-risk extraction
4. per-manager interop modernization

## Dependency Graph

### Group 0: Decision Freeze

- release architecture policy
- CLI compatibility policy

These decisions unblock the plan but do not edit source.

### Group 1: Foundation

- SDK-style project conversion
- assembly metadata mapping
- package cleanup
- contributor-doc updates
- published-build smoke test under WINE/Proton

This group is the first hard gate. If it fails, the rest of the plan should not proceed.

### Group 2: MainForm-Centered Cleanup

- duplicate event subscription fix
- `FormClosing` recursion fix
- `ProcessManager` disposal
- `InjectionManager` timer cleanup
- persistence-service extraction
- command-line parser extraction
- tests for extracted pure logic

This group is mostly sequential because `MainForm.cs` is the shared write surface.

### Group 3: Interop Modernization

- `ProcessManager` `[LibraryImport]`
- `InjectionManager` `[LibraryImport]`
- `MemoryManager` `[LibraryImport]`
- WINE/Proton regression gate

This group is the best place for parallel execution.

### Group 4: Optional Follow-Up

- interop deduplication
- nullable/style cleanup
- larger architecture refactors

These should stay out of the critical path.

## Real Parallelism

### Safe To Parallelize

- docs update while the Phase 1 smoke checklist is being prepared
- the three manager interop conversions after Phase 2 stabilizes

### Not Safe To Parallelize

- multiple extraction tasks that all edit `MainForm.cs`
- cleanup that changes shutdown behavior while interop signatures are also changing

## Corrections To The Original Plan

- WINE validation is a hard dependency, not a side verification task.
- Removing placeholder launch methods is not a safe migration task because profiles persist enum names.
- `win-x64` is not a settled architectural assumption.
- `LaunchOrchestrator` and broad architecture splitting are optional follow-up work, not required migration scope.
- Tests belong to extracted pure logic first, not to a speculative new core architecture.

## Recommended Task Order

1. Freeze architecture and CLI policy.
2. Convert project and publish successfully.
3. Prove the published build under WINE/Proton.
4. Fix form lifetime and cleanup bugs.
5. Extract persistence and CLI parsing.
6. Add tests for the extracted pure logic.
7. Convert interop declarations.
8. Run a second WINE/Proton regression gate.
9. Decide whether optional cleanup is still worth doing.
