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

### Local prerequisites (clean machine)

The committed manifest [`dev.crosshook.CrossHook.yml`](dev.crosshook.CrossHook.yml) sets `runtime-version` (currently `50`). The local helper [`scripts/build-flatpak.sh`](../../scripts/build-flatpak.sh) defaults `CROSSHOOK_FLATPAK_RUNTIME_VERSION` to the same value (`50` unless overridden). Install matching `org.gnome.Platform` and `org.gnome.Sdk` for that version before building.

**Packages / tools**

- **Required:** `flatpak`, `flatpak-builder` (most distros pull `ostree` as a dependency of the Flatpak stack).
- **Required for `./scripts/build-flatpak.sh --strict`:** `desktop-file-validate` (often in `desktop-file-utils`) and `appstreamcli` (often in `appstream` / `appstream-cli`, name varies by distro).

**Flathub and GNOME runtime/SDK**

Ensure the Flathub remote exists and install the Platform and Sdk that match the manifest (replace `50` if you bump the runtime):

```bash
flatpak remote-add --user --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
flatpak install --user flathub org.gnome.Platform//50 org.gnome.Sdk//50
```

Alternatively, `./scripts/build-flatpak.sh --install-deps` installs `flatpak` + `flatpak-builder` via your package manager, adds Flathub if missing, and runs the `flatpak install` for `org.gnome.Platform` and `org.gnome.Sdk` at version `${CROSSHOOK_FLATPAK_RUNTIME_VERSION:-50}`.

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
