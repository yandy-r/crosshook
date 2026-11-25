//! Renderer override and adapter log.
//!
//! gpui 0.3.8 on Linux always renders through wgpu. `WgpuContext::instance` builds its
//! `wgpu::Instance` with fixed `Backends::VULKAN | Backends::GL` and does not read
//! `WGPU_BACKEND`, so the only levers are the loader environment and gpui's own
//! `ZED_DEVICE_ID` adapter filter. `--renderer` maps onto those; an explicit env value wins.
//! The chosen adapter is logged as `SPIKE_RENDERER backend=… adapter=…` from gpui's own
//! "Selected GPU adapter" log line plus `Window::gpu_specs()`.

use std::sync::Mutex;

pub const CHOICES: &[&str] = &["vulkan", "gl", "lavapipe", "llvmpipe-gl"];

const LVP_ICD: &str = "/usr/share/vulkan/icd.d/lvp_icd.x86_64.json";
const MESA_EGL: &str = "/usr/share/glvnd/egl_vendor.d/50_mesa.json";

/// The env pairs a `--renderer` value maps to.
pub fn env_for(choice: &str) -> Result<Vec<(&'static str, &'static str)>, String> {
    Ok(match choice {
        // Default path; listed so the matrix names it explicitly.
        "vulkan" => vec![],
        // Hide every Vulkan ICD: wgpu falls back to its GL (EGL) backend.
        "gl" => vec![("VK_ICD_FILENAMES", "/dev/null")],
        // Mesa lavapipe only: a CPU Vulkan adapter (wgpu DeviceType::Cpu).
        "lavapipe" => vec![("VK_ICD_FILENAMES", LVP_ICD)],
        // GL through Mesa's EGL vendor with llvmpipe, Vulkan hidden (bench E3-sw recipe).
        "llvmpipe-gl" => vec![
            ("VK_ICD_FILENAMES", "/dev/null"),
            ("__EGL_VENDOR_LIBRARY_FILENAMES", MESA_EGL),
            ("LIBGL_ALWAYS_SOFTWARE", "1"),
        ],
        other => {
            return Err(format!(
                "unknown --renderer {other:?}; expected one of {CHOICES:?}"
            ));
        }
    })
}

/// Parses `--NAME X` / `--NAME=X` from argv.
pub fn flag(args: &[String], name: &str) -> Result<Option<String>, String> {
    let long = format!("--{name}");
    let eq = format!("--{name}=");
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        if let Some(v) = arg.strip_prefix(&eq) {
            return Ok(Some(v.to_owned()));
        }
        if *arg == long {
            return it
                .next()
                .cloned()
                .map(Some)
                .ok_or_else(|| format!("{long} needs a value"));
        }
    }
    Ok(None)
}

/// Applies `--renderer` and `--device` (PCI device id, gpui's `ZED_DEVICE_ID`) before gpui
/// starts. Must run before any thread is spawned.
pub fn apply(args: &[String]) -> Result<(), String> {
    let mut pairs = Vec::new();
    if let Some(choice) = flag(args, "renderer")? {
        pairs.extend(env_for(&choice)?);
    }
    let device = flag(args, "device")?;
    for (key, value) in pairs {
        set_if_unset(key, value);
    }
    if let Some(device) = device {
        set_if_unset("ZED_DEVICE_ID", &device);
    }
    Ok(())
}

fn set_if_unset(key: &str, value: &str) {
    if std::env::var_os(key).is_none() {
        // SAFETY: called at the top of `main`, before gpui or any other thread starts.
        unsafe { std::env::set_var(key, value) };
    }
}

static SELECTED: Mutex<Option<String>> = Mutex::new(None);

/// Minimal logger: warnings and errors from every crate, plus gpui's adapter-selection info
/// lines (kept for `SPIKE_RENDERER`). `SPIKE_LOG=info` prints every info line.
struct SpikeLogger {
    verbose: bool,
}

impl log::Log for SpikeLogger {
    fn enabled(&self, meta: &log::Metadata<'_>) -> bool {
        meta.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record<'_>) {
        let msg = record.args().to_string();
        if msg.starts_with("Selected GPU adapter:") {
            *SELECTED.lock().unwrap_or_else(|e| e.into_inner()) = Some(msg.clone());
        }
        let gpu = record.target().contains("wgpu");
        if record.level() <= log::Level::Warn || self.verbose || (gpu && renderer_log_on()) {
            eprintln!("[{} {}] {msg}", record.level(), record.target());
        }
    }

    fn flush(&self) {}
}

pub fn renderer_log_on() -> bool {
    std::env::var_os("SPIKE_LOG_RENDERER").is_some() || crate::bench::enabled()
}

pub fn init_logger() {
    let verbose = std::env::var("SPIKE_LOG").as_deref() == Ok("info");
    if log::set_boxed_logger(Box::new(SpikeLogger { verbose })).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
}

/// `backend=Vulkan adapter="NVIDIA …" software=false driver="…"`.
pub fn describe(specs: Option<gpui::GpuSpecs>) -> String {
    let selected = SELECTED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_default();
    let backend = selected
        .rsplit_once('(')
        .map_or("unknown", |(_, b)| b.trim_end_matches(')'));
    match specs {
        Some(s) => format!(
            "backend={backend} adapter={:?} software={} driver={:?} driver_info={:?}",
            s.device_name, s.is_software_emulated, s.driver_name, s.driver_info
        ),
        None => format!("backend={backend} adapter=unknown"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_choices() {
        assert!(env_for("vulkan").unwrap().is_empty());
        assert_eq!(
            env_for("gl").unwrap(),
            vec![("VK_ICD_FILENAMES", "/dev/null")]
        );
        assert_eq!(env_for("llvmpipe-gl").unwrap().len(), 3);
        assert!(env_for("metal").is_err());
    }

    #[test]
    fn parses_both_flag_forms() {
        let a = |v: &[&str]| v.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        assert_eq!(
            flag(&a(&["x", "--renderer", "gl"]), "renderer")
                .unwrap()
                .as_deref(),
            Some("gl")
        );
        assert_eq!(
            flag(&a(&["--device=0x2d18"]), "device").unwrap().as_deref(),
            Some("0x2d18")
        );
        assert_eq!(flag(&a(&["x"]), "renderer").unwrap(), None);
        assert!(flag(&a(&["--renderer"]), "renderer").is_err());
    }
}
