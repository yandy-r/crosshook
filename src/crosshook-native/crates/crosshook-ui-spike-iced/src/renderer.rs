//! Renderer override: `--renderer wgpu-vulkan|wgpu-gl|tiny-skia` maps onto iced's own env
//! switches (`ICED_BACKEND`, read in `iced_core::backend`, and `WGPU_BACKEND`, read by
//! `wgpu::Backends::from_env`). An explicit env value set by the user wins over the flag.

pub const CHOICES: &[&str] = &["wgpu-vulkan", "wgpu-gl", "tiny-skia"];

/// The env pairs a `--renderer` value maps to.
pub fn env_for(choice: &str) -> Result<Vec<(&'static str, &'static str)>, String> {
    Ok(match choice {
        "wgpu-vulkan" => vec![("ICED_BACKEND", "wgpu"), ("WGPU_BACKEND", "vulkan")],
        "wgpu-gl" => vec![("ICED_BACKEND", "wgpu"), ("WGPU_BACKEND", "gl")],
        "tiny-skia" => vec![("ICED_BACKEND", "tiny-skia")],
        other => {
            return Err(format!(
                "unknown --renderer {other:?}; expected one of {CHOICES:?}"
            ));
        }
    })
}

/// Parses `--renderer X` / `--renderer=X` from argv.
pub fn from_args(args: &[String]) -> Result<Option<String>, String> {
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        if let Some(v) = arg.strip_prefix("--renderer=") {
            return Ok(Some(v.to_owned()));
        }
        if arg == "--renderer" {
            return it
                .next()
                .cloned()
                .map(Some)
                .ok_or_else(|| "--renderer needs a value".into());
        }
    }
    Ok(None)
}

/// Applies the flag before iced starts. Must run before any thread is spawned.
pub fn apply(args: &[String]) -> Result<(), String> {
    let Some(choice) = from_args(args)? else {
        return Ok(());
    };
    for (key, value) in env_for(&choice)? {
        if std::env::var_os(key).is_none() {
            // SAFETY: called at the top of `main`, before iced or any other thread starts.
            unsafe { std::env::set_var(key, value) };
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_choices() {
        assert_eq!(
            env_for("tiny-skia").unwrap(),
            vec![("ICED_BACKEND", "tiny-skia")]
        );
        assert_eq!(env_for("wgpu-gl").unwrap()[1], ("WGPU_BACKEND", "gl"));
        assert!(env_for("metal").is_err());
    }

    #[test]
    fn parses_both_flag_forms() {
        let a = |v: &[&str]| v.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        assert_eq!(
            from_args(&a(&["x", "--renderer", "wgpu-gl"]))
                .unwrap()
                .as_deref(),
            Some("wgpu-gl")
        );
        assert_eq!(
            from_args(&a(&["--renderer=tiny-skia"])).unwrap().as_deref(),
            Some("tiny-skia")
        );
        assert_eq!(from_args(&a(&["x"])).unwrap(), None);
        assert!(from_args(&a(&["--renderer"])).is_err());
    }
}
