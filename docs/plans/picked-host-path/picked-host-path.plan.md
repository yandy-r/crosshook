# Plan: picked host path resolution (YAN-855 / M1-36)

Binding decisions: [`decisions.md`](decisions.md). Paths are relative to `src/crosshook-native/`.

## Acceptance criteria → verification

| AC                                                                                                                                                                                                         | Verification                                                                                                                             |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| Unit tests cover `/run/host` (root, nested, duplicate slashes), doc-portal via xattr, doc-portal via D-Bus fallback, unresolved doc-portal error, Windows-style and relative pass-through, and empty input | `cargo test -p crosshook-core platform::tests::picked_path`                                                                              |
| `dialog.ts` contains no path-normalization logic                                                                                                                                                           | Code review, plus `src/utils/__tests__/dialog.test.ts` checking that every pick is forwarded to `normalize_host_path` with its `pathUse` |
| Manual Flatpak portal pick produces a host path and the game launches                                                                                                                                      | Not runnable here. Filed as a Linear follow-up (D10)                                                                                     |

## B0: contracts (frozen)

```rust
// crates/crosshook-core/src/platform/picked_path.rs
pub enum HostPathError { Empty, UnresolvedDocumentPortalPath { path: String }, InvalidPortalReply { path: String } }
// Display + std::error::Error, hand-written (no thiserror in this module, matching portals/)
pub trait XattrReader { fn document_portal_host_path(&self, path: &str) -> Option<String>; }
#[async_trait::async_trait]
pub trait DocumentsPortal: Send + Sync { async fn get_host_paths(&self, doc_ids: &[String]) -> Result<HashMap<String, Vec<u8>>, String>; }
pub struct SystemXattrReader;          // wraps detect::read_document_portal_host_path_xattr
pub struct ZbusDocumentsPortal;        // session bus, org.freedesktop.portal.Documents, /org/freedesktop/portal/documents, 2 s timeout
pub async fn resolve_picked_host_path(path: &str, xattr: &dyn XattrReader, portal: &dyn DocumentsPortal) -> Result<PathBuf, HostPathError>;
pub async fn resolve_picked_host_path_default(path: &str) -> Result<PathBuf, HostPathError>;
pub fn resolve_picked_sandbox_path(path: &str) -> String; // doc-portal paths unchanged; otherwise normalize_flatpak_host_path
pub enum PickedPathUse { Host, Sandbox } // serde, rename_all = "lowercase"
```

Re-exported from `platform/mod.rs`.

```rust
// src-tauri/src/commands/shared.rs
#[tauri::command]
pub async fn normalize_host_path(path: String, path_use: PickedPathUse) -> Result<String, String>
```

```ts
// src/utils/dialog.ts
export type PickedPathUse = 'host' | 'sandbox';
chooseFile(title, use, filters?) / chooseDirectory(title, use) / chooseSaveFile(title, use, options?)
// -> callCommand('normalize_host_path', { path, pathUse: use }); on error alert + null
```

## B1 (parallel, disjoint files)

- **Rust**: `picked_path.rs`, `platform/mod.rs`, `platform/tests/{mod.rs,picked_path.rs}`, `detect.rs` doc comment, `src-tauri/src/commands/shared.rs`, ADR-0001 row.
- **TS**: `dialog.ts`, every caller listed in D6, `lib/mocks/handlers/system.ts`, the new `utils/__tests__/dialog.test.ts`, and the test files that mock `chooseFile`/`chooseDirectory` signatures.

## Gate

`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p crosshook-core`, `npm run typecheck`, `npm test`, `./scripts/lint.sh`, `./scripts/check-mock-coverage.sh`.
