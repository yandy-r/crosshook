//! GPU vendor detection from the kernel's DRM sysfs class.
//!
//! `/sys/class/drm` holds one entry per card (`card0`), render node
//! (`renderD128`) and connector (`card0-eDP-1`), next to a plain `version`
//! file. Cards and render nodes carry a `device` symlink to their PCI device,
//! whose `vendor` attribute is the PCI vendor id (`0x10de` is NVIDIA).
//! Connectors link to their card instead and `version` is not a directory, so
//! those entries have no `device/vendor` and are skipped.
//!
//! The probe is a few small read-only sysfs reads, cheap enough for the
//! pre-runtime startup path: it spawns nothing (no `lspci`, no `nvidia-smi`)
//! and so stays outside the host-tool gateway. `/sys/class` and `/sys/devices`
//! are mounted read-only inside the Flatpak sandbox, so the same path answers
//! there.

use std::path::Path;

/// sysfs class directory the kernel lists its DRM devices in.
const DRM_CLASS_ROOT: &str = "/sys/class/drm";

/// PCI vendor id of NVIDIA Corporation.
const NVIDIA_VENDOR_ID: &str = "0x10de";

/// Returns `true` when an NVIDIA GPU is among the DRM devices in
/// `/sys/class/drm`, including hybrid setups (an Intel or AMD GPU next to a
/// discrete NVIDIA one). Returns `false` when none is found or the directory
/// is missing or unreadable.
pub fn has_nvidia_gpu() -> bool {
    has_nvidia_gpu_in(Path::new(DRM_CLASS_ROOT))
}

/// [`has_nvidia_gpu`] against another DRM class directory, for tests.
pub(crate) fn has_nvidia_gpu_in(drm_root: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(drm_root) else {
        return false;
    };
    for entry in entries.flatten() {
        let Ok(vendor) = std::fs::read_to_string(entry.path().join("device").join("vendor")) else {
            continue;
        };
        if vendor.trim().eq_ignore_ascii_case(NVIDIA_VENDOR_ID) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::tempdir;

    use super::{has_nvidia_gpu, has_nvidia_gpu_in};

    /// A DRM entry (`card0`, `renderD128`) whose `device/vendor` reads `vendor`.
    fn drm_device(root: &Path, name: &str, vendor: &str) {
        let vendor_file = root.join(name).join("device").join("vendor");
        fs::create_dir_all(vendor_file.parent().expect("parent directory"))
            .expect("create device directory");
        fs::write(&vendor_file, vendor).expect("write vendor");
    }

    /// An entry with a `device` link but no `vendor` attribute (a connector, or
    /// a non-PCI platform device).
    fn drm_device_without_vendor(root: &Path, name: &str) {
        fs::create_dir_all(root.join(name).join("device")).expect("create device directory");
    }

    #[test]
    fn detects_nvidia_card() {
        let root = tempdir().expect("temp dir");
        drm_device(root.path(), "card0", "0x10de\n");
        assert!(has_nvidia_gpu_in(root.path()));
    }

    #[test]
    fn detects_uppercase_vendor_with_crlf() {
        let root = tempdir().expect("temp dir");
        drm_device(root.path(), "card0", "0X10DE\r\n");
        assert!(has_nvidia_gpu_in(root.path()));
    }

    #[test]
    fn detects_vendor_with_surrounding_whitespace_and_mixed_case() {
        let root = tempdir().expect("temp dir");
        drm_device(root.path(), "card0", "  0x10De \t\n");
        assert!(has_nvidia_gpu_in(root.path()));
    }

    #[test]
    fn ignores_amd_and_intel_cards() {
        let root = tempdir().expect("temp dir");
        drm_device(root.path(), "card0", "0x1002\n");
        drm_device(root.path(), "card1", "0x8086\n");
        drm_device(root.path(), "renderD128", "0x1002\n");
        assert!(!has_nvidia_gpu_in(root.path()));
    }

    #[test]
    fn detects_nvidia_in_hybrid_setup_in_either_order() {
        let root = tempdir().expect("temp dir");
        drm_device(root.path(), "card0", "0x8086\n");
        drm_device(root.path(), "renderD128", "0x8086\n");
        drm_device(root.path(), "card1", "0x10de\n");
        drm_device(root.path(), "renderD129", "0x10de\n");
        assert!(has_nvidia_gpu_in(root.path()));

        let root = tempdir().expect("temp dir");
        drm_device(root.path(), "card0", "0x10de\n");
        drm_device(root.path(), "card1", "0x1002\n");
        assert!(has_nvidia_gpu_in(root.path()));
    }

    #[test]
    fn detects_nvidia_render_node_without_a_card_entry() {
        let root = tempdir().expect("temp dir");
        drm_device(root.path(), "renderD129", "0x10de\n");
        assert!(has_nvidia_gpu_in(root.path()));
    }

    #[test]
    fn skips_entries_without_a_readable_vendor() {
        let root = tempdir().expect("temp dir");
        // The plain `version` file of the DRM class: not a directory.
        fs::write(root.path().join("version"), "drm 1.1.0 20160902\n").expect("write version");
        // A connector whose `device` link leads to its card, not a PCI device.
        drm_device_without_vendor(root.path(), "card0-eDP-1");
        // A card whose vendor attribute exists but is empty.
        drm_device(root.path(), "card0", "");
        assert!(!has_nvidia_gpu_in(root.path()));

        drm_device(root.path(), "card1", "0x10de\n");
        assert!(has_nvidia_gpu_in(root.path()));
    }

    #[test]
    fn does_not_match_near_miss_values() {
        for vendor in ["nvidia", "10de", "0x10de0", "0x10d", "0x10de 0x2684"] {
            let root = tempdir().expect("temp dir");
            drm_device(root.path(), "card0", vendor);
            assert!(
                !has_nvidia_gpu_in(root.path()),
                "{vendor:?} must not count as NVIDIA"
            );
        }
    }

    #[test]
    fn returns_false_for_empty_directory() {
        let root = tempdir().expect("temp dir");
        assert!(!has_nvidia_gpu_in(root.path()));
    }

    #[test]
    fn returns_false_for_missing_directory() {
        let root = tempdir().expect("temp dir");
        assert!(!has_nvidia_gpu_in(&root.path().join("drm")));
    }

    #[test]
    fn returns_false_when_root_is_not_a_directory() {
        let root = tempdir().expect("temp dir");
        fs::write(root.path().join("drm"), "not a directory").expect("write file");
        assert!(!has_nvidia_gpu_in(&root.path().join("drm")));
    }

    #[test]
    fn default_entry_point_answers_without_panicking() {
        // The answer depends on the host GPU; only the contract is checked.
        let _ = has_nvidia_gpu();
    }
}
