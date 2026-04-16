# Analogical Reasoning: Sandboxed Apps with Host Tool Dependencies

> **Perspective**: Cross-domain analogies — what patterns emerge when sandboxed applications need tools that live on the host?
>
> **Date**: 2026-04-15

---

## Executive Summary

Across desktop platforms — Flatpak, Snap, macOS App Sandbox, Windows MSIX, and Android — a consistent set of patterns emerges when sandboxed applications need host-side tools. Every domain grapples with the same fundamental tension: **isolation for reliability vs. integration for capability**. The most successful apps don't pick one extreme — they build a layered strategy that detects capabilities, delegates to the host when needed, bundles selectively, and degrades gracefully. CrossHook's situation (a Flatpak orchestrator that needs Wine/Proton, gamescope, MangoHud, winetricks, etc. on the host) maps directly onto patterns already battle-tested by VS Code, Bottles, Podman Desktop, and GNOME Boxes.

**Confidence**: High — patterns are well-documented across 10+ production applications with years of field experience.

---

## 1. VS Code Flatpak — The IDE That Needs Everything

### The Problem

VS Code as a Flatpak cannot access host-installed compilers, language servers, debuggers, or SDKs. The sandbox makes the integrated terminal nearly useless out of the box since host binaries are not on `PATH`.

### How It Solves It

| Strategy                      | Implementation                                                                               | Trade-off                                                            |
| ----------------------------- | -------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| **`flatpak-spawn --host`**    | Terminal profile configured to use `/usr/bin/flatpak-spawn --host bash`                      | Breaks sandbox model; requires `--talk-name=org.freedesktop.Flatpak` |
| **SDK Extensions**            | `org.freedesktop.Sdk.Extension.dotnet`, `.golang`, etc. enabled via `FLATPAK_ENABLE_SDK_EXT` | Limited selection; version lag behind upstream                       |
| **Toolbox/Podman containers** | Remote SSH/Containers extension connects to a development container                          | Adds complexity layer; great for power users                         |
| **Wrapper scripts**           | Language servers wrapped with `flatpak-spawn --forward-fd=1 --host --watch-bus`              | Per-tool manual setup; fragile                                       |

### Outcome

Many users conclude the VS Code Flatpak is **not practical for development** and switch to the native `.deb`/`.rpm` package. One prominent blog post asks: "who can actually use this Flatpak, if native compilation is all but impossible?"

### Transferable Lesson

> **When your app's core value depends on host tools, bundling-only fails.** VS Code proves that even generous SDK extensions can't cover the long tail of developer needs. The escape hatch (`flatpak-spawn --host`) becomes the de facto solution, effectively defeating the sandbox.

**Confidence**: High — multiple independent sources confirm; community consensus is clear.

**Sources:**

- [Flathub VS Code manifest](https://github.com/flathub/com.visualstudio.code)
- [Cogitri: Using VSCode in a Flatpak](https://www.cogitri.dev/posts/06-using-flatpaked-vscode/)
- [benzblog: The VS Code Flatpak is useless](https://bentsukun.ch/posts/vscode-flatpak/)
- [Radu Zaharia: Using the VSCode Flatpak](https://blog.raduzaharia.com/using-the-vscode-flatpak-distribution-a275d59ff1c7)

---

## 2. Podman Desktop Flatpak — GUI Wrapper for a Host Daemon

### The Problem

Podman Desktop is a graphical frontend. The actual container runtime (`podman`, `crun`/`runc`) **must** run on the host — you cannot meaningfully run containers inside a Flatpak sandbox.

### How It Solves It

- **Socket delegation**: The Flatpak connects to the host's Podman socket (`$XDG_RUNTIME_DIR/podman/podman.sock`) via filesystem override
- **`podman-remote`**: Instead of calling `podman` directly, the sandboxed app uses `podman-remote` which speaks the REST API to the host daemon
- **First-run onboarding wizard**: A proposed (and partially implemented) setup flow that detects whether Podman is installed on the host, offers to help install it, and walks through configuration step-by-step
- **Extension system**: Podman Desktop supports plugins/extensions for additional container engines

### Outcome

Works well in practice. The key insight: Podman Desktop **never pretends to be self-contained**. It explicitly positions itself as a GUI for a host service.

### Transferable Lesson

> **The "thin GUI + host delegation" pattern works when the app is transparent about it.** Podman Desktop succeeds because it embraces socket-based delegation rather than fighting it. Its onboarding wizard detects host state and guides users through setup — exactly the pattern CrossHook needs.

**Confidence**: High — Podman Desktop is actively maintained, widely used, and well-documented.

**Sources:**

- [Podman Desktop on Flathub](https://flathub.org/en/apps/io.podman_desktop.PodmanDesktop)
- [Podman Desktop GitHub](https://github.com/podman-desktop/podman-desktop)
- [Podman Desktop Onboarding Discussion #3244](https://github.com/containers/podman-desktop/discussions/3244)

---

## 3. GNOME Boxes Flatpak — Bundle the Whole Stack

### The Problem

GNOME Boxes needs libvirt, QEMU, and spice-gtk. These are complex, version-sensitive system services.

### How It Solves It

**Full bundling**: The Flatpak runs its **own** libvirt and QEMU inside the sandbox. It does not connect to the host's `libvirtd`.

### What Breaks

| Lost Capability              | Reason                                       |
| ---------------------------- | -------------------------------------------- |
| Bridged networking           | Requires host-level network config           |
| Physical device pass-through | Sandbox prevents device access               |
| `virt-manager` interop       | Sandboxed libvirt is invisible to host tools |
| TPM emulation                | `swtpm` not included in runtime              |
| Host VM management           | Cannot connect to host's `libvirtd`          |

### Outcome

GNOME Boxes explicitly targets **end-users** who want a self-contained VM experience. Developer Felipe Borges: "Since Boxes targets end-users we really treat it as a self-contained app, whereas we don't expect users to be tweaking it under the hood." Users who need advanced features use native packages instead.

### Transferable Lesson

> **Full bundling works only when you can accept the lost capabilities.** GNOME Boxes proves that bundling the entire stack is viable for a simplified use case, but the feature ceiling drops dramatically. For CrossHook, where the whole point is orchestrating host-level game launching (Proton, gamescope, etc.), this "bundle everything" approach would defeat the app's purpose.

**Confidence**: High — well-documented design decision by the GNOME team.

**Sources:**

- [Felipe Borges: Boxes + Flatpak](https://blogs.gnome.org/feborges/boxes-flatpak/)
- [GNOME Boxes Flathub Issues](https://github.com/flathub/org.gnome.Boxes/issues/99)
- [Ctrl blog: GNOME Boxes review](https://www.ctrl.blog/entry/review-gnome-boxes.html)

---

## 4. Firefox/Chromium Flatpak — The Codec Extension Pattern

### The Problem

Browsers need media codecs (H.264, H.265, VP9) and hardware-accelerated video decoding (VA-API). Patent-encumbered codecs can't ship in the base Freedesktop runtime.

### How It Solves It

- **Flatpak extension**: `org.freedesktop.Platform.ffmpeg-full` provides the full codec set as an optional, separately installable extension
- **Browser flags**: Users toggle `about:config` settings (`media.ffmpeg.vaapi.enabled`) to activate VA-API
- **Environment overrides**: `flatpak override --user --env=...` for GPU driver compatibility

### Outcome

Works, but requires manual configuration. The experience is measurably worse than native packages — users report thermal issues from software decoding before they discover the extension.

### Transferable Lesson

> **The "optional Flatpak extension" pattern is the right mechanism for supplementary tools, but discoverability is the killer.** Users don't know the extension exists until they hit the problem. Firefox's approach lacks a built-in "you're missing codec support, install this" prompt. CrossHook should learn from this: if you use extensions, build discovery and guidance into the app itself.

**Confidence**: High — extensively documented across Flathub, Mozilla Bugzilla, and community forums.

**Sources:**

- [Flathub Discourse: Enable video hardware acceleration on Flatpak Firefox](https://discourse.flathub.org/t/how-to-enable-video-hardware-acceleration-on-flatpak-firefox/3125)
- [Mozilla Bug 1628407](https://bugzilla.mozilla.org/show_bug.cgi?id=1628407)
- [DEV Community: Firefox Flatpak VA-API on Wayland](https://dev.to/archerallstars/enable-video-hardware-acceleration-in-firefox-flatpak-on-wayland-1m7m)

---

## 5. GIMP Flatpak — The Plugin Extension Point Pattern

### The Problem

GIMP has a rich plugin ecosystem. Plugins may need native libraries (e.g., G'MIC, LiquidRescale) that can't be dropped into a user directory. The Flatpak sandbox isolates GIMP from host-installed plugins.

### How It Solves It

**Two-tier approach:**

1. **Simple plugins**: Drop scripts/binaries into `~/.var/app/org.gimp.GIMP/config/GIMP/2.10/plug-ins/` — works inside sandbox
2. **Complex plugins (with native deps)**: Packaged as Flatpak extensions (`org.gimp.GIMP.Plugin.GMic`, `.Resynthesizer`, etc.) and distributed via Flathub

The GIMP manifest declares an `org.gimp.GIMP.Plugin` extension point, and plugin maintainers build against it.

### Outcome

Works well for the curated set of plugins (5 official extensions). But plugin authors must learn Flatpak packaging, and version coupling (GIMP API version changes require plugin rebuilds) creates maintenance burden.

### Transferable Lesson

> **Extension points work best for a curated, small set of integrations with dedicated maintainers.** GIMP's model is sustainable because there are ~5 complex plugins that matter. CrossHook's tool set (gamescope, MangoHud, winetricks, etc.) is similarly small and well-defined — making this pattern a strong fit. The key risk is maintenance: who rebuilds the extension when the runtime updates?

**Confidence**: High — GIMP's Flatpak plugin system is production-proven and well-documented.

**Sources:**

- [GIMP Developer: Flatpak plugin publishing](https://testing.developer.gimp.org/resource/distributing-plug-ins/flatpak-plugin-publishing/)
- [Flathub GIMP manifest](https://github.com/flathub/org.gimp.GIMP)
- [discuss.pixls.us: GIMP Flatpak plugins](https://discuss.pixls.us/t/gimp-flatpak-how-to-install-plugins/17177)

---

## 6. Bottles Flatpak — The Closest Analogy to CrossHook

### The Problem

Bottles manages Wine/Proton environments and needs optional host tools (MangoHud, gamescope, vkBasalt, OBS) — **nearly identical** to CrossHook's situation.

### How It Solves It

| Tool        | Integration Method                           | Install Command                                                     |
| ----------- | -------------------------------------------- | ------------------------------------------------------------------- |
| MangoHud    | Flatpak Vulkan layer extension               | `flatpak install org.freedesktop.Platform.VulkanLayer.MangoHud`     |
| gamescope   | Flatpak Vulkan layer extension               | `flatpak install org.freedesktop.Platform.VulkanLayer.gamescope`    |
| vkBasalt    | Flatpak Vulkan layer extension               | `flatpak install org.freedesktop.Platform.VulkanLayer.vkBasalt`     |
| OBS Capture | Flatpak Vulkan layer extension               | `flatpak install org.freedesktop.Platform.VulkanLayer.OBSVkCapture` |
| Wine/Proton | Downloaded and managed by Bottles internally | Runners downloaded at runtime                                       |
| winetricks  | Available within sandbox shell               | Run from Flatpak sandbox                                            |

Bottles adds these extensions to its `PATH`:

- `/usr/lib/extensions/vulkan/MangoHud/bin/`
- `/usr/lib/extensions/vulkan/gamescope/bin/`
- `/usr/lib/extensions/vulkan/OBSVkCapture/bin/`

### Known Problems

1. **Discoverability is poor**: Tooltips telling users to install extensions are only visible on hover, and contain long terminal commands users must type manually
2. **GitHub Issue #2008**: "[Request] Tell the user to install optional components through Flatpak" — users want in-app guidance
3. **GitHub Issue #2801**: "[Request] Suggest how to install missing Flatpak extensions" — the app should detect and suggest
4. **Host vs. Flatpak confusion**: Installing MangoHud via RPM/DEB does NOT work with Bottles Flatpak — only the Flatpak extension works, and users don't understand why

### Transferable Lesson

> **Bottles is CrossHook's nearest analogue and its pain points are CrossHook's roadmap.** The Vulkan layer extension pattern for MangoHud/gamescope is proven, but the UX around discovering and installing them is the weak link. CrossHook should implement what Bottles users are requesting: in-app detection of missing tools, clear install instructions, and a guided first-run setup.

**Confidence**: High — direct analogical match with documented user frustration and feature requests.

**Sources:**

- [Bottles Issue #2008: Tell user to install optional components](https://github.com/bottlesdevs/Bottles/issues/2008)
- [Bottles Issue #2801: Suggest missing Flatpak extensions](https://github.com/bottlesdevs/Bottles/issues/2801)
- [Bottles Forum: Flatpak MangoHud config](https://forum.usebottles.com/t/flatpak-mangohub-and-config-files/365)
- [Fedora Discussion: Gamescope, MangoHud as Flatpaks](https://discussion.fedoraproject.org/t/gamescope-mangohud-etc-as-flatpaks/133803)

---

## 7. macOS App Sandbox — Embedding Helper Tools

### The Problem

macOS sandboxed apps (distributed via App Store) cannot access Homebrew, Xcode CLI tools, or arbitrary host binaries. Yet many apps need helper tools for their core function.

### How It Solves It

- **Embedded helper tools**: Apple provides an official pattern for [embedding command-line tools in sandboxed apps](https://developer.apple.com/documentation/xcode/embedding-a-helper-tool-in-a-sandboxed-app) — the tool is bundled inside the `.app` bundle with its own entitlements
- **XPC Services**: Privileged operations delegate to XPC helper processes that run outside the sandbox with elevated permissions
- **Hardened runtime + notarization**: Bundled binaries must be signed and notarized individually
- **"Install Xcode CLI Tools" prompt**: macOS itself implements the canonical "install checker" pattern — when an app needs developer tools, the OS shows a system dialog offering to download and install them

### Transferable Lesson

> **macOS proves that "detect + prompt + install" is a platform-blessed pattern.** The `xcode-select --install` flow is exactly what CrossHook should emulate: detect the missing tool, show a clear prompt, offer a one-click (or one-command) install path. Apple validates this as the right UX for tool dependencies.

**Confidence**: High — official Apple documentation and universal developer experience.

**Sources:**

- [Apple: Embedding a helper tool in a sandboxed app](https://developer.apple.com/documentation/xcode/embedding-a-helper-tool-in-a-sandboxed-app)
- [Apple Forums: Sandboxed app with additional binaries](https://developer.apple.com/forums/thread/129672)

---

## 8. Windows MSIX/AppX — Dynamic Dependencies

### The Problem

Windows MSIX packages run in lightweight app containers with file system and registry virtualization. Apps may need framework packages or runtime dependencies that aren't bundled.

### How It Solves It

- **`<Dependencies>` manifest element**: Declares required OS versions and framework packages
- **Microsoft Store auto-resolution**: When installing from the Store, dependencies are automatically downloaded and installed
- **Dynamic Dependency API** (Windows 11): Apps can declare runtime dependencies on MSIX framework packages at runtime:
  1. Declare dependency criteria (version, architecture)
  2. Specify a lifetime artifact (process, file, or registry key) so the OS knows when the dependency is no longer needed
  3. Request access at runtime, adding a run-time reference
  4. Release the reference when done
- **Full-trust escape hatch**: Desktop Bridge apps can run as `DesktopBridge` with full trust, bypassing the container entirely

### Transferable Lesson

> **Windows MSIX's Dynamic Dependency API is the most sophisticated "declare + resolve + use + release" pattern.** The lifetime artifact concept (tracking whether a dependency is still needed) is elegant. CrossHook could implement a simpler version: detect tool → check if available → request use → handle absence gracefully.

**Confidence**: Medium — MSIX is well-documented but the Dynamic Dependency API is relatively new and less field-tested than Flatpak extensions.

**Sources:**

- [Microsoft Learn: Dynamic Dependency API](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/framework-packages/use-the-dynamic-dependency-api)
- [Microsoft Learn: How packaged desktop apps run](https://learn.microsoft.com/en-us/windows/msix/desktop/desktop-to-uwp-behind-the-scenes)
- [TechEngage: AppX to MSIX Evolution (2026)](https://techengage.com/windows-app-packaging-appx-msix-evolution/)

---

## 9. Snap Packages — The Alternative Sandbox

### The Problem

Snap faces the same bundling-vs-host-access tension as Flatpak but with different architectural choices.

### Key Differences from Flatpak

| Aspect                | Snap                                                | Flatpak                                |
| --------------------- | --------------------------------------------------- | -------------------------------------- |
| **Sandbox mechanism** | AppArmor                                            | bubblewrap (Linux namespaces)          |
| **Host integration**  | More system-integrated; interface-based permissions | Stricter isolation by default          |
| **Shared runtimes**   | Each snap bundles all deps                          | Apps share runtimes (smaller installs) |
| **Store model**       | Centralized (Canonical, proprietary backend)        | Decentralized (Flathub, open)          |
| **Startup time**      | Slower (compressed squashfs mount)                  | Faster (shared runtimes)               |
| **Permission model**  | `plugs` and `slots` interfaces                      | Static permissions + portals           |

### Snap's "Interface" Pattern

Snap uses `plugs` (what the app needs) and `slots` (what the host provides) to manage host access. This is more explicit than Flatpak's filesystem overrides — each capability is a named interface (`network`, `home`, `camera`, `hardware-observe`, etc.).

### Transferable Lesson

> **Snap's explicit plug/slot interface model makes capabilities discoverable and auditable.** CrossHook's tool detection system should similarly enumerate capabilities (gamescope: available/missing, MangoHud: available/missing) as named, typed interfaces rather than opaque filesystem checks.

**Confidence**: Medium — Snap is well-documented but CrossHook targets Flatpak, so the lessons are architectural rather than directly implementable.

**Sources:**

- [Glukhov: Snap vs Flatpak Ultimate Guide 2025](https://www.glukhov.org/post/2025/12/snap-vs-flatpack/)
- [It's FOSS: Flatpak vs Snap — 10 Differences](https://itsfoss.com/flatpak-vs-snap/)
- [How-To Geek: APT vs Snap vs Flatpak](https://www.howtogeek.com/apt-vs-snap-vs-flatpak-ubuntu-package-managers-explained/)

---

## 10. Android — Dynamic Feature Modules & Companion Apps

### The Problem

Android apps may need optional capabilities that shouldn't bloat the base install, or may depend on functionality provided by a separate app.

### How It Solves It

**Pattern 1: Dynamic Feature Modules (DFM)**

- Features are packaged as separate modules delivered on demand via Google Play
- Users download only when they need the feature
- The base app detects module availability and presents install UI
- Implementation uses `SplitCompat` for seamless code loading

**Pattern 2: Companion App Detection**

- App checks whether a required companion app is installed (via `PackageManager` or `CapabilityClient`)
- If missing, shows a prompt to install from the Play Store
- Deep-links to the exact Play Store listing

### Transferable Lesson

> **Android's two-tier model (in-app optional modules + cross-app dependency detection) maps directly to CrossHook's needs.** Flatpak extensions are analogous to Dynamic Feature Modules (optional capabilities bundled in the ecosystem). Host tools are analogous to companion apps (external dependencies detected and guided to install). The key UX pattern: detect → inform → deep-link to install → verify.

**Confidence**: Medium — the analogy is conceptually clean but the implementation mechanisms differ significantly between Android and Flatpak.

**Sources:**

- [Android Developers: Play Feature Delivery](https://developer.android.com/guide/playcore/feature-delivery)
- [Android Developers: On-demand delivery](https://developer.android.com/guide/playcore/feature-delivery/on-demand)
- [Android Developers: Standalone vs non-standalone Wear OS apps](https://developer.android.com/training/wearables/apps/standalone-apps)

---

## Cross-Cutting Pattern Analysis

### Pattern 1: Detect + Prompt + Install (The Universal Pattern)

Every successful analogy implements some form of this:

| App               | Detection                                 | Prompt              | Install Path             |
| ----------------- | ----------------------------------------- | ------------------- | ------------------------ |
| macOS (Xcode CLI) | Automatic on first use                    | System dialog       | One-click download       |
| Podman Desktop    | First-run wizard                          | Step-by-step UI     | Platform package manager |
| Bottles           | Toggle hover tooltip                      | Tooltip text (weak) | Manual terminal command  |
| Android           | `PackageManager` / `CapabilityClient`     | In-app dialog       | Play Store deep link     |
| GIMP              | N/A (extensions just appear if installed) | N/A                 | Manual `flatpak install` |

**CrossHook should**: Implement the strongest version — automatic detection with in-app prompts and copy-to-clipboard install commands, similar to macOS's `xcode-select` but adapted for Flatpak.

### Pattern 2: Extension Points for Curated Tools

Works when:

- The set of tools is **small and well-defined** (GIMP: ~5 plugins; Bottles: ~4 Vulkan layers)
- Someone is willing to **maintain** the extensions
- The tools are **platform-ecosystem native** (Flatpak extensions for Flatpak apps)

Fails when:

- The tool set is **open-ended** (VS Code: infinite language servers)
- Extensions **lag upstream** versions
- Users expect to use **host-installed** versions

**CrossHook's tools** (gamescope, MangoHud, winetricks, vkBasalt) fit the "curated and small" profile. Flatpak extensions are viable.

### Pattern 3: Socket/API Delegation

Used by:

- Podman Desktop (Podman socket)
- Flatpak portals (D-Bus APIs)
- VS Code Remote (SSH/container API)

Appropriate when:

- The host provides a **daemon or service**
- Communication can happen over a **well-defined protocol**
- The sandboxed app is a **frontend/orchestrator**

**CrossHook relevance**: Limited. Game launching doesn't happen through a daemon — it's direct process execution. `flatpak-spawn --host` is closer to what CrossHook needs.

### Pattern 4: Graceful Degradation

How apps handle missing capabilities:

| App                 | Missing Capability    | Behavior                                |
| ------------------- | --------------------- | --------------------------------------- |
| Firefox Flatpak     | ffmpeg-full extension | Software decoding (slow, hot laptop)    |
| GNOME Boxes Flatpak | Host libvirtd         | No bridged networking (works otherwise) |
| VS Code Flatpak     | Host compilers        | Terminal is useless; IDE still opens    |
| Bottles Flatpak     | MangoHud extension    | Feature toggle greyed out               |

**CrossHook should**: Follow Bottles' pattern — grey out unavailable features with an explanation and install link, rather than failing silently or crashing.

### Pattern 5: "Just Use Native Packages" Escape Valve

Every analogy has users who abandon the Flatpak and install natively:

- VS Code: "just use the .deb"
- GNOME Boxes: "just `apt install qemu-kvm libvirt-daemon`"
- Firefox: "just use the distro package"

**CrossHook should**: Accept this reality. The Flatpak should be the best experience possible, but the native package (AppImage) should remain first-class. Don't force Flatpak-only features.

---

## Recommendations for CrossHook

Based on the cross-domain analysis, ranked by confidence and impact:

### Tier 1: High Confidence, High Impact

1. **Implement "Detect + Prompt + Guide" for all host dependencies**
   - At first launch and periodically, check for gamescope, MangoHud, Proton, winetricks, umu-launcher
   - Show clear status (installed/missing/outdated) in a settings or status panel
   - Provide copy-to-clipboard install commands for each missing tool
   - _Analogues_: macOS Xcode CLI prompt, Podman Desktop onboarding, Android companion app detection

2. **Use Flatpak Vulkan layer extensions for MangoHud, gamescope, vkBasalt**
   - These already exist: `org.freedesktop.Platform.VulkanLayer.MangoHud`, `.gamescope`, `.vkBasalt`
   - Declare them as optional extensions in the CrossHook Flatpak manifest
   - _Analogue_: Bottles (exact same pattern and toolset)

3. **Grey out unavailable features with explanatory UI**
   - When MangoHud isn't installed, show the overlay toggle as disabled with "Install MangoHud extension to enable"
   - Don't hide features — show them as available-but-needing-setup
   - _Analogues_: Bottles (partially), Android DFM install prompts

### Tier 2: Medium Confidence, Medium Impact

4. **Build a first-run onboarding wizard**
   - Detect host environment (Steam installed? Proton available? gamescope? MangoHud?)
   - Present a checklist with status indicators
   - Offer guided install for each missing component
   - _Analogues_: Podman Desktop onboarding wizard, OpenClaw CLI wizard

5. **Use `flatpak-spawn --host` as the delegation mechanism for tool execution**
   - CrossHook already does this — validate that it's the right long-term pattern
   - Ensure `--talk-name=org.freedesktop.Flatpak` is in the manifest
   - _Analogues_: VS Code Flatpak, host-spawn tool

### Tier 3: Lower Confidence, Consider Later

6. **Explore custom Flatpak extension point for CrossHook-specific tools**
   - If winetricks or other tools aren't available as standard extensions, consider creating a `com.crosshook.Extension.*` namespace
   - _Analogue_: GIMP's `org.gimp.GIMP.Plugin.*` pattern
   - _Risk_: Maintenance burden of building/updating extensions

7. **Don't bundle Wine/Proton inside the Flatpak**
   - GNOME Boxes proves full bundling works for simplified use cases, but CrossHook needs the full capability set
   - Wine/Proton should remain host-managed (via Steam, umu-launcher, or manual install)
   - _Analogue_: GNOME Boxes (negative example — shows the limitations)

---

## Anti-Patterns to Avoid

| Anti-Pattern                           | Example                               | Why It Fails                                        |
| -------------------------------------- | ------------------------------------- | --------------------------------------------------- |
| **Silent failure**                     | Firefox: no codec, just slow video    | Users don't know what's wrong or how to fix it      |
| **Tooltip-only guidance**              | Bottles: hover to see install command | Too hidden; users never discover it                 |
| **Assuming host tools exist**          | VS Code Flatpak terminal              | Breaks immediately; user blames the app             |
| **Full stack bundling**                | GNOME Boxes libvirt                   | Loses critical host-integration features            |
| **Ignoring Flatpak-vs-host confusion** | Bottles MangoHud RPM vs Flatpak       | Users install the wrong version and it doesn't work |

---

## Summary Matrix

| Analogy           | Pattern Used                                | Outcome                        | CrossHook Applicability                          |
| ----------------- | ------------------------------------------- | ------------------------------ | ------------------------------------------------ |
| VS Code Flatpak   | `flatpak-spawn --host` + SDK extensions     | Mixed (many users leave)       | Host delegation: YES. SDK extensions: LIMITED    |
| Podman Desktop    | Socket delegation + onboarding wizard       | Good                           | Onboarding wizard: STRONG FIT                    |
| GNOME Boxes       | Full bundling                               | Works for simple cases         | DO NOT emulate for CrossHook                     |
| Firefox Flatpak   | Optional codec extension                    | Works but poor discoverability | Extension pattern: YES. Discoverability: IMPROVE |
| GIMP Flatpak      | Plugin extension points                     | Good for curated set           | Extension points: GOOD FIT for small tool set    |
| Bottles Flatpak   | Vulkan layer extensions                     | Good technically, poor UX      | CLOSEST ANALOGUE. Copy pattern, fix UX           |
| macOS App Sandbox | Embedded helpers + system install prompts   | Excellent                      | Detect+prompt pattern: ADOPT                     |
| Windows MSIX      | Dynamic dependencies + manifest declaration | Sophisticated                  | Lifecycle management concepts: ADAPT             |
| Snap              | Plug/slot interfaces                        | More integrated                | Named capability model: ADAPT conceptually       |
| Android           | DFM + companion detection                   | Mature ecosystem               | Two-tier optional/external model: ADAPT          |

---

## Uncertainties and Gaps

1. **Flatpak extension version coupling**: How does the `org.freedesktop.Platform.VulkanLayer.MangoHud` extension handle runtime version bumps? Is CrossHook exposed to breakage when the Freedesktop runtime updates? _Needs investigation._

2. **`flatpak-spawn --host` security posture**: Flathub may tighten requirements around `--talk-name=org.freedesktop.Flatpak` in the future. The April 2026 sandbox escape CVE (CVE-2026-34078) shows this is an active attack surface. _Needs monitoring._

3. **umu-launcher in Flatpak**: How does umu-launcher (CrossHook's Proton orchestrator) interact with Flatpak sandboxing? Is it available as a Flatpak extension? _Not covered by analogies — needs direct investigation._

4. **Onboarding wizard implementation complexity**: The Podman Desktop onboarding discussion reveals that detecting host state from inside a Flatpak sandbox is non-trivial, especially across distros. _Needs prototyping._
