# gpui_tokio (vendored)

Source: <https://github.com/zed-industries/zed/tree/279fe070bb389b79652e52065b2f001edcc0b11b/crates/gpui_tokio>
(`crates/gpui_tokio/src/gpui_tokio.rs` at zed@279fe07, the commit `gpui-pre 0.3.8` snapshots).

Copyright Zed Industries, Inc. Licensed under the Apache License, Version 2.0 (`LICENSE-APACHE`
in this directory, copied from the Zed repository root, which the crate's own `LICENSE-APACHE`
symlinks to).

Changes for the CrossHook spike:

- Compiled as a module of `crosshook-ui-spike-gpui` (`#[path]` include) instead of a separate
  crate, so `gpui`/`gpui_util` resolve to the spike's `gpui-pre`/`gpui-pre-util` =0.3.8 renames.
- No functional change to `init`, `init_from_handle`, `Tokio::spawn`, `Tokio::spawn_result` or
  `Tokio::handle`. The spike only calls `init_from_handle` with the app-owned runtime.
