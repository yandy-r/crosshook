use std::collections::{BTreeMap, HashMap};

use serde::Deserialize;

use super::models::{
    ProtonDbAdvisoryKind, ProtonDbAdvisoryNote, ProtonDbEnvVarSuggestion,
    ProtonDbLaunchOptionSuggestion, ProtonDbRecommendationGroup,
};

const RESERVED_ENV_KEYS: &[&str] = &[
    "WINEPREFIX",
    "STEAM_COMPAT_DATA_PATH",
    "STEAM_COMPAT_CLIENT_INSTALL_PATH",
];
const MAX_ENV_GROUPS: usize = 3;
const MAX_LAUNCH_GROUPS: usize = 3;
const MAX_NOTE_GROUPS: usize = 4;
const MAX_GROUP_NOTES: usize = 3;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ProtonDbReportFeedResponse {
    #[serde(default)]
    pub reports: Vec<ProtonDbReportEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ProtonDbReportEntry {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub timestamp: i64,
    #[serde(default)]
    pub responses: ProtonDbReportResponses,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtonDbReportResponses {
    #[serde(default)]
    pub concluding_notes: String,
    #[serde(default)]
    pub launch_options: String,
    #[serde(default)]
    pub proton_version: String,
    #[serde(default)]
    pub variant: String,
    #[serde(default)]
    pub notes: ProtonDbReportNotes,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct ProtonDbReportNotes {
    #[serde(default)]
    pub variant: String,
}

#[derive(Default)]
struct EnvGroupAggregate {
    count: usize,
    env_vars: Vec<ProtonDbEnvVarSuggestion>,
    /// Per raw launch string: how many reports contributed that exact string (copy-only tails).
    launch_options: BTreeMap<String, usize>,
    notes: Vec<ProtonDbAdvisoryNote>,
}

#[derive(Default)]
struct LaunchGroupAggregate {
    count: usize,
    notes: Vec<ProtonDbAdvisoryNote>,
}

pub(crate) fn degraded_recommendation_group(message: &str) -> ProtonDbRecommendationGroup {
    ProtonDbRecommendationGroup {
        group_id: "report-feed-unavailable".to_string(),
        title: "Community recommendations unavailable".to_string(),
        summary: message.to_string(),
        ..ProtonDbRecommendationGroup::default()
    }
}

pub(crate) fn normalize_report_feed(
    feed: ProtonDbReportFeedResponse,
) -> Vec<ProtonDbRecommendationGroup> {
    let mut env_groups = BTreeMap::<String, EnvGroupAggregate>::new();
    let mut launch_groups = BTreeMap::<String, LaunchGroupAggregate>::new();
    let mut note_counts = BTreeMap::<String, usize>::new();

    for report in feed.reports {
        let source_label = source_label(&report);
        let note_text = normalize_text(&report.responses.concluding_notes);
        let raw_launch = normalize_text(&report.responses.launch_options);
        let env_vars = safe_env_var_suggestions(&raw_launch, &source_label);

        if !env_vars.is_empty() {
            let signature = env_group_signature(&env_vars);
            let entry = env_groups.entry(signature).or_default();
            entry.count += 1;
            if entry.env_vars.is_empty() {
                entry.env_vars = env_vars;
                for env_var in &mut entry.env_vars {
                    env_var.supporting_report_count = Some(entry.count as u32);
                }
            }
            if !raw_launch.is_empty() && launch_string_needs_copy_only(&raw_launch) {
                *entry.launch_options.entry(raw_launch.clone()).or_insert(0) += 1;
            }
            if !note_text.is_empty() {
                push_note(&mut entry.notes, &source_label, &note_text);
            }
            continue;
        }

        if !raw_launch.is_empty() {
            let entry = launch_groups.entry(raw_launch.clone()).or_default();
            entry.count += 1;
            if !note_text.is_empty() {
                push_note(&mut entry.notes, &source_label, &note_text);
            }
            continue;
        }

        if !note_text.is_empty() {
            *note_counts.entry(note_text).or_insert(0) += 1;
        }
    }

    let mut groups = Vec::new();

    let mut env_entries = env_groups.into_iter().collect::<Vec<_>>();
    env_entries.sort_by(|left, right| right.1.count.cmp(&left.1.count).then(left.0.cmp(&right.0)));
    for (index, (_, mut aggregate)) in env_entries.into_iter().take(MAX_ENV_GROUPS).enumerate() {
        for env_var in &mut aggregate.env_vars {
            env_var.supporting_report_count = Some(aggregate.count as u32);
        }

        groups.push(ProtonDbRecommendationGroup {
            group_id: format!("supported-env-{}", index + 1),
            title: "Suggested environment variables".to_string(),
            summary: format!(
                "Seen in {} ProtonDB report{}.",
                aggregate.count,
                if aggregate.count == 1 { "" } else { "s" }
            ),
            notes: aggregate.notes,
            env_vars: aggregate.env_vars,
            launch_options: aggregate
                .launch_options
                .into_iter()
                .map(|(text, count)| ProtonDbLaunchOptionSuggestion {
                    kind: ProtonDbAdvisoryKind::LaunchOption,
                    source_label: "Raw launch option".to_string(),
                    text,
                    supporting_report_count: Some(count as u32),
                })
                .collect(),
        });
    }

    let mut launch_entries = launch_groups.into_iter().collect::<Vec<_>>();
    launch_entries
        .sort_by(|left, right| right.1.count.cmp(&left.1.count).then(left.0.cmp(&right.0)));
    for (index, (launch, aggregate)) in launch_entries
        .into_iter()
        .take(MAX_LAUNCH_GROUPS)
        .enumerate()
    {
        groups.push(ProtonDbRecommendationGroup {
            group_id: format!("copy-only-launch-{}", index + 1),
            title: "Copy-only launch string".to_string(),
            summary: format!(
                "Seen in {} ProtonDB report{}.",
                aggregate.count,
                if aggregate.count == 1 { "" } else { "s" }
            ),
            notes: aggregate.notes,
            launch_options: vec![ProtonDbLaunchOptionSuggestion {
                kind: ProtonDbAdvisoryKind::LaunchOption,
                source_label: "Launch option".to_string(),
                text: launch,
                supporting_report_count: Some(aggregate.count as u32),
            }],
            ..ProtonDbRecommendationGroup::default()
        });
    }

    let mut notes = note_counts.into_iter().collect::<Vec<_>>();
    notes.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
    if !notes.is_empty() {
        groups.push(ProtonDbRecommendationGroup {
            group_id: "community-notes".to_string(),
            title: "Community notes".to_string(),
            summary: "Recent ProtonDB notes from community reports.".to_string(),
            notes: notes
                .into_iter()
                .take(MAX_NOTE_GROUPS)
                .map(|(text, count)| ProtonDbAdvisoryNote {
                    kind: ProtonDbAdvisoryKind::Note,
                    source_label: format!("{count} report{}", if count == 1 { "" } else { "s" }),
                    text,
                })
                .collect(),
            ..ProtonDbRecommendationGroup::default()
        });
    }

    groups
}

fn source_label(report: &ProtonDbReportEntry) -> String {
    let custom_variant = normalize_text(&report.responses.notes.variant);
    if !custom_variant.is_empty() {
        return format!("Custom Proton: {custom_variant}");
    }

    let variant = normalize_text(&report.responses.variant);
    if !variant.is_empty() && variant != "official" {
        return format!("Variant: {variant}");
    }

    let proton_version = normalize_text(&report.responses.proton_version);
    if !proton_version.is_empty() {
        return format!("Proton {proton_version}");
    }

    if !report.id.trim().is_empty() {
        return format!("Report {}", report.id.trim());
    }

    if report.timestamp > 0 {
        return format!("Report {}", report.timestamp);
    }

    "ProtonDB report".to_string()
}

fn push_note(notes: &mut Vec<ProtonDbAdvisoryNote>, source_label: &str, text: &str) {
    if notes.iter().any(|note| note.text == text) || notes.len() >= MAX_GROUP_NOTES {
        return;
    }

    notes.push(ProtonDbAdvisoryNote {
        kind: ProtonDbAdvisoryKind::Note,
        source_label: source_label.to_string(),
        text: text.to_string(),
    });
}

fn normalize_text(value: &str) -> String {
    value.trim().replace('\0', "")
}

fn env_group_signature(env_vars: &[ProtonDbEnvVarSuggestion]) -> String {
    env_vars
        .iter()
        .map(|env| format!("{}={}", env.key, env.value))
        .collect::<Vec<_>>()
        .join("\n")
}

fn safe_env_var_suggestions(raw_launch: &str, source_label: &str) -> Vec<ProtonDbEnvVarSuggestion> {
    let prefix = raw_launch
        .split("%command%")
        .next()
        .unwrap_or(raw_launch)
        .trim();
    if prefix.is_empty() {
        return Vec::new();
    }

    let mut env_map: HashMap<String, ProtonDbEnvVarSuggestion> = HashMap::new();
    for token in prefix.split_whitespace() {
        let Some((key, value)) = token.split_once('=') else {
            continue;
        };
        let normalized_key = key.trim();
        if !is_safe_env_key(normalized_key) || !is_safe_env_value(value) {
            continue;
        }
        if RESERVED_ENV_KEYS.contains(&normalized_key)
            || normalized_key.starts_with("STEAM_COMPAT_")
        {
            continue;
        }
        env_map.insert(
            normalized_key.to_string(),
            ProtonDbEnvVarSuggestion {
                key: normalized_key.to_string(),
                value: value.to_string(),
                source_label: source_label.to_string(),
                supporting_report_count: None,
            },
        );
    }

    let mut env_vars: Vec<ProtonDbEnvVarSuggestion> = env_map.into_values().collect();
    env_vars.sort_by(|left, right| left.key.cmp(&right.key).then(left.value.cmp(&right.value)));
    env_vars
}

fn is_safe_env_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(ch) if ch == '_' || ch.is_ascii_uppercase() => {}
        _ => return false,
    }

    chars.all(|ch| ch == '_' || ch.is_ascii_uppercase() || ch.is_ascii_digit())
}

fn is_safe_env_value(value: &str) -> bool {
    if value.contains('\0') {
        return false;
    }

    !value.chars().any(|ch| {
        ch.is_whitespace()
            || matches!(
                ch,
                '$' | ';' | '"' | '\'' | '\\' | '`' | '|' | '&' | '<' | '>' | '(' | ')' | '%'
            )
    })
}

fn launch_string_needs_copy_only(raw_launch: &str) -> bool {
    let prefix = raw_launch
        .split("%command%")
        .next()
        .unwrap_or(raw_launch)
        .trim();
    if prefix.is_empty() {
        return false;
    }

    prefix
        .split_whitespace()
        .any(|token| token.split_once('=').is_none() || token.contains('"') || token.contains('\''))
}
