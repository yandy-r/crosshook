use std::fs;
use std::path::Path;

use crate::prefix_deps::PrefixDepsError;

#[derive(Debug, Default, PartialEq, Eq)]
struct WindowsVersionRegistryFields {
    current_build: Option<String>,
    current_version: Option<String>,
    product_name: Option<String>,
}

pub(super) fn snapshot_prefix_windows_version(
    prefix_path: &Path,
) -> Result<String, PrefixDepsError> {
    let registry_path = prefix_path.join("system.reg");
    let registry = fs::read_to_string(&registry_path).map_err(|error| {
        PrefixDepsError::ValidationError(format!(
            "could not read prefix Windows version registry: {error}"
        ))
    })?;
    let mut fields = WindowsVersionRegistryFields::default();
    let mut in_current_version = false;

    for line in registry.lines() {
        if line.starts_with('[') {
            in_current_version =
                line.starts_with(r#"[Software\\Microsoft\\Windows NT\\CurrentVersion]"#);
            continue;
        }
        if !in_current_version {
            continue;
        }

        if let Some(value) = parse_registry_string(line, "CurrentBuild") {
            fields.current_build = Some(value.to_string());
        } else if let Some(value) = parse_registry_string(line, "CurrentVersion") {
            fields.current_version = Some(value.to_string());
        } else if let Some(value) = parse_registry_string(line, "ProductName") {
            fields.product_name = Some(value.to_string());
        }
    }

    windows_version_verb(&fields)
        .map(str::to_string)
        .ok_or_else(|| {
            PrefixDepsError::ValidationError(
            "could not derive a supported Windows compatibility version from the prefix registry"
                .to_string(),
        )
        })
}

fn parse_registry_string<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let value = line.strip_prefix(&format!(r#""{key}"="#))?;
    value.strip_prefix('"')?.strip_suffix('"')
}

fn windows_version_verb(fields: &WindowsVersionRegistryFields) -> Option<&'static str> {
    let product_name = fields.product_name.as_deref().map(str::to_ascii_lowercase);
    if let Some(product_name) = product_name.as_deref() {
        if product_name.contains("server 2008 r2") {
            return Some("win2k8r2");
        }
        if product_name.contains("server 2008") {
            return Some("win2k8");
        }
        if product_name.contains("server 2003") {
            return Some("win2k3");
        }
    }

    if let Some(build) = fields
        .current_build
        .as_deref()
        .and_then(|build| build.parse::<u32>().ok())
    {
        match build {
            22_000.. => return Some("win11"),
            10_240.. => return Some("win10"),
            9_600 => return Some("win81"),
            9_200 => return Some("win8"),
            7_600 | 7_601 => return Some("win7"),
            _ => {}
        }
    }

    if let Some(product_name) = product_name.as_deref() {
        let verb = if product_name.contains("windows 8.1") {
            Some("win81")
        } else if product_name.contains("windows 8") {
            Some("win8")
        } else if product_name.contains("windows 7") {
            Some("win7")
        } else if product_name.contains("windows vista") {
            Some("vista")
        } else if product_name.contains("windows xp") {
            Some("winxp")
        } else if product_name.contains("windows 2000") {
            Some("win2k")
        } else {
            None
        };
        if verb.is_some() {
            return verb;
        }
    }

    match fields.current_version.as_deref() {
        Some("6.3") => Some("win81"),
        Some("6.2") => Some("win8"),
        Some("6.1") => Some("win7"),
        Some("6.0") => Some("vista"),
        Some("5.2") => Some("win2k3"),
        Some("5.1") => Some("winxp"),
        Some("5.0") => Some("win2k"),
        _ => None,
    }
}

/// Whether `verb` is a winetricks Windows compatibility-version verb.
pub fn is_windows_version_verb(verb: &str) -> bool {
    matches!(
        verb,
        "nt351"
            | "nt40"
            | "vista"
            | "win10"
            | "win11"
            | "win20"
            | "win2k"
            | "win2k3"
            | "win2k8"
            | "win2k8r2"
            | "win30"
            | "win31"
            | "win7"
            | "win8"
            | "win81"
            | "win95"
            | "win98"
            | "winme"
            | "winxp"
    )
}
