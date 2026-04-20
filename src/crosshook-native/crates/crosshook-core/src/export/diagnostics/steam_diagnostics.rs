use crate::steam;

pub(super) fn collect_steam_diagnostics() -> (String, String, usize) {
    let mut diagnostics = Vec::new();
    let root_candidates = steam::discover_steam_root_candidates("", &mut diagnostics);

    let mut lines = Vec::new();
    lines.push("=== Steam Root Candidates ===".to_string());
    if root_candidates.is_empty() {
        lines.push("(none found)".to_string());
    } else {
        for root in &root_candidates {
            lines.push(format!("  {}", root.display()));
        }
    }
    lines.push(String::new());

    lines.push("=== Discovery Diagnostics ===".to_string());
    if diagnostics.is_empty() {
        lines.push("(no diagnostics)".to_string());
    } else {
        for diagnostic in &diagnostics {
            lines.push(format!("  {diagnostic}"));
        }
    }

    let mut proton_diagnostics = Vec::new();
    let proton_installs = steam::discover_compat_tools(&root_candidates, &mut proton_diagnostics);
    let proton_count = proton_installs.len();

    if !proton_diagnostics.is_empty() {
        lines.push(String::new());
        lines.push("=== Proton Discovery Diagnostics ===".to_string());
        for diagnostic in &proton_diagnostics {
            lines.push(format!("  {diagnostic}"));
        }
    }

    let proton_json =
        serde_json::to_string_pretty(&proton_installs).unwrap_or_else(|_| "[]".to_string());

    (lines.join("\n"), proton_json, proton_count)
}
