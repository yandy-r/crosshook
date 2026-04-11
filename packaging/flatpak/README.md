# Flatpak Packaging and CI Notes

This directory contains the committed Flatpak packaging assets for CrossHook:

- `dev.crosshook.CrossHook.yml`
- `dev.crosshook.CrossHook.desktop`
- `dev.crosshook.CrossHook.metainfo.xml`

The current runtime target is GNOME `50` (`org.gnome.Platform//50` and `org.gnome.Sdk//50`).

## CI Build Flow (Phase 2)

Release automation is defined in `.github/workflows/release.yml`:

- `build-flatpak` builds the bundle with `flatpak/flatpak-github-actions/flatpak-builder@v6`
- CI validates metadata before building:
  - `desktop-file-validate packaging/flatpak/dev.crosshook.CrossHook.desktop`
  - `appstreamcli validate packaging/flatpak/dev.crosshook.CrossHook.metainfo.xml`
- The publish job attaches the Flatpak bundle to the GitHub Release alongside AppImage and CLI assets

## GNOME Runtime Upgrade Path

When a new stable GNOME runtime is adopted, update all runtime-coupled locations in the same PR.

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
