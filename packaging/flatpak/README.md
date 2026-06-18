# Flatpak Packaging and CI Notes

This directory contains the committed Flatpak packaging assets for CrossHook:

- `dev.crosshook.CrossHook.yml`
- `dev.crosshook.CrossHook.desktop`
- `dev.crosshook.CrossHook.metainfo.xml`

The manifest also consumes generated icon inputs staged by
[`scripts/build-flatpak.sh`](../../scripts/build-flatpak.sh):

- `assets/icon-128.png`
- `assets/icon-256.png`
- `assets/icon-512.png`

Those PNGs are generated from SVG sources by
[`scripts/generate-assets.sh`](../../scripts/generate-assets.sh). They remain
Flatpak packaging inputs for local builds and CI release staging.

The current runtime target is GNOME `50` (`org.gnome.Platform//50` and `org.gnome.Sdk//50`).

Current CrossHook releases publish Flatpak bundles only. The local Flatpak helper builds the
production Tauri release binary, stages the Flatpak inputs, and produces an installable bundle.

```bash
./scripts/build-flatpak.sh --rebuild --strict
flatpak install --user --reinstall ~/.local/share/crosshook/artifacts/CrossHook_amd64.flatpak
flatpak run dev.crosshook.CrossHook
```

For a one-command local update flow, use:

```bash
./scripts/build-flatpak.sh --rebuild --install --strict
flatpak run dev.crosshook.CrossHook
```

## CI Build Flow (Phase 2)

Release automation is defined in `.github/workflows/release.yml` and runs only from tags pushed to
the GitHub `github` remote:

- `build-flatpak` builds the bundle with `flatpak/flatpak-github-actions/flatpak-builder@v6`
- CI validates metadata before building:
  - `desktop-file-validate packaging/flatpak/dev.crosshook.CrossHook.desktop`
  - `appstreamcli validate packaging/flatpak/dev.crosshook.CrossHook.metainfo.xml`
- The publish job attaches the Flatpak bundle to the GitHub Release

## GNOME Runtime Upgrade Path

When a new stable GNOME runtime is adopted, update all runtime-coupled locations in the same PR.

### Local prerequisites (clean machine)

The committed manifest [`dev.crosshook.CrossHook.yml`](dev.crosshook.CrossHook.yml) sets `runtime-version` (currently `50`). The local helper [`scripts/build-flatpak.sh`](../../scripts/build-flatpak.sh) defaults `CROSSHOOK_FLATPAK_RUNTIME_VERSION` to the same value (`50` unless overridden), and CI uses the matching `ghcr.io/flathub-infra/flatpak-github-actions:gnome-50` image. Install matching `org.gnome.Platform` and `org.gnome.Sdk` for that version before building.

**Packages / tools**

- **Required:** `flatpak`, `flatpak-builder` (most distros pull `ostree` as a dependency of the Flatpak stack).
- **Required when generated icons are missing:** `rsvg-convert` (often in `librsvg2-bin` / `librsvg2-tools` / `librsvg`) and ImageMagick (`magick` or `convert`) so `scripts/generate-assets.sh` can produce `assets/icon-128.png`, `assets/icon-256.png`, and `assets/icon-512.png`.
- **Required for `./scripts/build-flatpak.sh --strict`:** `desktop-file-validate` (often in `desktop-file-utils`) and `appstreamcli` (often in `appstream` / `appstream-cli`, name varies by distro).

**Flathub and GNOME runtime/SDK**

Ensure the Flathub remote exists and install the Platform and Sdk that match the manifest (replace `50` if you bump the runtime):

```bash
flatpak remote-add --user --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
flatpak install --user flathub org.gnome.Platform//50 org.gnome.Sdk//50
```

Alternatively, `./scripts/build-flatpak.sh --install-deps` installs `flatpak`, `flatpak-builder`, `rsvg-convert`, and ImageMagick via your package manager, adds Flathub if missing, and runs the `flatpak install` for `org.gnome.Platform` and `org.gnome.Sdk` at version `${CROSSHOOK_FLATPAK_RUNTIME_VERSION:-50}`.

**Smoke test**

After a successful `./scripts/build-flatpak.sh --strict`, install and run the bundle using the commands in step 5 below.

1. **Manifest runtime**  
   Update `runtime-version` in `packaging/flatpak/dev.crosshook.CrossHook.yml`.
2. **CI container image**  
   Update the release workflow container image tag (`ghcr.io/flathub-infra/flatpak-github-actions:gnome-<major>`) in `.github/workflows/release.yml`.
3. **Local build script default**  
   Keep `CROSSHOOK_FLATPAK_RUNTIME_VERSION` default in `scripts/build-flatpak.sh` aligned with the manifest.
4. **Revalidate packaging metadata**  
   Run:
   - `desktop-file-validate packaging/flatpak/dev.crosshook.CrossHook.desktop`
   - `appstreamcli validate packaging/flatpak/dev.crosshook.CrossHook.metainfo.xml`
5. **Smoke test the upgraded runtime**  
   Build and install locally:
   - `./scripts/build-flatpak.sh --strict`
   - `flatpak install --user --reinstall <bundle-path>`
   - `flatpak run dev.crosshook.CrossHook`

## Runtime Drift Note

Older tracking text referenced GNOME runtime `48`. The source of truth is now runtime `50` in the committed manifest and CI container tag.

## Shared mode (advanced users)

By default, the Flatpak build uses **per-app data isolation** (data lives under
`~/.var/app/dev.crosshook.CrossHook/{config,cache,data}/`). On first launch, CrossHook
imports existing legacy AppImage-era host data from `~/.config/crosshook/` and
`~/.local/share/crosshook/` into the sandbox one-way. This preserves existing user
profiles and metadata during the move to Flatpak-only releases; it is not ongoing
dual-distribution support. See
[ADR-0004](../../docs/architecture/adr-0004-flatpak-per-app-isolation.md) for
details.

If you prefer the Flatpak to keep using the host XDG data directory instead of
the default per-app sandbox directory, opt in persistently:

```bash
flatpak override --user --env=CROSSHOOK_FLATPAK_HOST_XDG=1 dev.crosshook.CrossHook
```

This is **not** how Flathub-distributed installs behave by default, and it is
**not** documented on Flathub. Use at your own risk.

To revert to the default isolated mode:

```bash
flatpak override --user --unset-env=CROSSHOOK_FLATPAK_HOST_XDG dev.crosshook.CrossHook
```
