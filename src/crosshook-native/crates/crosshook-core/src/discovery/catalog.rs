//! Cross-tap trainer catalog DTOs and the pure filter/facet/rank/page pass
//! (Forgejo #27). Serves both the SQLite path and the degraded filesystem
//! path so filter/facet/ordering semantics are byte-identical.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::ranking::{catalog_match_score, catalog_sort_key, COMPATIBILITY_BAND_ORDER};

pub const CATALOG_DEFAULT_LIMIT: u32 = 50;
pub const CATALOG_MAX_LIMIT: u32 = 200;
pub const CATALOG_TITLE_FACET_CAP: usize = 100;

const LOADING_MODE_ORDER: [&str; 3] = ["source_directory", "copy_to_prefix", "unknown"];

/// IPC input: text query + per-dimension facet selections + paging.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CatalogQuery {
    pub query: Option<String>,
    pub game_titles: Vec<String>,
    pub loading_modes: Vec<String>,
    pub compatibility_bands: Vec<String>,
    pub tap_urls: Vec<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Get Trainer payload attached to a catalog entry (guides, never hosts).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSource {
    pub source_name: String,
    pub source_url: String,
    pub sha256: Option<String>,
    pub trainer_version: Option<String>,
    pub game_version: Option<String>,
    pub notes: Option<String>,
}

/// One community profile row in the aggregated cross-tap catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    /// DB rowid; `None` in degraded mode. NOT a list key.
    pub id: Option<i64>,
    pub tap_url: String,
    pub tap_local_path: String,
    pub relative_path: String,
    /// Empty for source-only entries (no community profile to import).
    pub manifest_path: String,
    pub game_name: Option<String>,
    pub game_version: Option<String>,
    pub trainer_name: Option<String>,
    pub trainer_version: Option<String>,
    pub proton_version: Option<String>,
    pub compatibility_rating: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    /// Space-joined; the frontend splits.
    pub platform_tags: Option<String>,
    /// `None` buckets to the "unknown" facet value.
    pub trainer_loading_mode: Option<String>,
    pub schema_version: i64,
    pub sources: Vec<CatalogSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FacetValue {
    pub value: String,
    pub count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogFacets {
    pub game_titles: Vec<FacetValue>,
    pub loading_modes: Vec<FacetValue>,
    pub compatibility_bands: Vec<FacetValue>,
    pub taps: Vec<FacetValue>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPage {
    pub entries: Vec<CatalogEntry>,
    pub facets: CatalogFacets,
    /// Post-filter, pre-pagination.
    pub total_count: i64,
    /// Distinct taps in the unfiltered corpus.
    pub tap_count: i64,
    pub degraded: bool,
}

/// Internal catalog row input carrying tap identity for source grouping.
/// Never crosses IPC.
#[derive(Debug, Clone)]
pub struct CatalogRowInput {
    pub tap_id: String,
    pub entry: CatalogEntry,
}

/// Internal trainer-source input carrying tap identity for source grouping.
/// Never crosses IPC. `tap_url` / `tap_local_path` seed synthesized
/// source-only entries when no profile row matches the group.
#[derive(Debug, Clone)]
pub struct CatalogSourceInput {
    pub tap_id: String,
    pub tap_url: String,
    pub tap_local_path: String,
    pub game_name: String,
    pub source: CatalogSource,
}

fn band(entry: &CatalogEntry) -> &str {
    match entry.compatibility_rating.as_deref() {
        Some(rating @ ("platinum" | "working" | "partial" | "broken")) => rating,
        _ => "unknown",
    }
}

fn mode(entry: &CatalogEntry) -> &str {
    match entry.trainer_loading_mode.as_deref() {
        Some(mode @ ("source_directory" | "copy_to_prefix")) => mode,
        _ => "unknown",
    }
}

fn matches_text(entry: &CatalogEntry, query: &str) -> bool {
    if catalog_match_score(
        entry.game_name.as_deref(),
        entry.trainer_name.as_deref(),
        query,
    ) > 0
    {
        return true;
    }
    let secondary = [
        entry.author.as_deref(),
        entry.description.as_deref(),
        entry.platform_tags.as_deref(),
        entry.game_version.as_deref(),
        entry.trainer_version.as_deref(),
        entry.proton_version.as_deref(),
        Some(entry.tap_url.as_str()),
        Some(entry.manifest_path.as_str()),
    ];
    if secondary
        .into_iter()
        .flatten()
        .any(|haystack| haystack.to_lowercase().contains(query))
    {
        return true;
    }
    entry.sources.iter().any(|source| {
        source.source_name.to_lowercase().contains(query)
            || source
                .notes
                .as_deref()
                .is_some_and(|notes| notes.to_lowercase().contains(query))
    })
}

fn matches_game_titles(entry: &CatalogEntry, selections: &[String]) -> bool {
    selections.is_empty()
        || entry.game_name.as_deref().is_some_and(|name| {
            let name = name.to_lowercase();
            selections
                .iter()
                .any(|selection| selection.to_lowercase() == name)
        })
}

fn matches_value(value: &str, selections: &[String]) -> bool {
    selections.is_empty() || selections.iter().any(|selection| selection == value)
}

/// Order facet values by count DESC then value ASC (case-insensitive, then
/// exact).
fn sort_facet_values(values: &mut [FacetValue]) {
    values.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.value.to_lowercase().cmp(&b.value.to_lowercase()))
            .then_with(|| a.value.cmp(&b.value))
    });
}

/// Count facet values over `rows`, pin selected values at zero, and order by
/// count DESC then value ASC (case-insensitive, then exact).
fn ranked_facet(counts: HashMap<String, i64>, selections: &[String]) -> Vec<FacetValue> {
    let mut counts = counts;
    for selection in selections {
        counts.entry(selection.clone()).or_insert(0);
    }
    let mut values: Vec<FacetValue> = counts
        .into_iter()
        .map(|(value, count)| FacetValue { value, count })
        .collect();
    sort_facet_values(&mut values);
    values
}

/// Title facet: counts are grouped case-insensitively (lowercased key), the
/// displayed value is the first-seen casing, and selections pin at zero.
fn ranked_title_facet(
    counts: HashMap<String, (String, i64)>,
    selections: &[String],
) -> Vec<FacetValue> {
    let mut counts = counts;
    for selection in selections {
        counts
            .entry(selection.to_lowercase())
            .or_insert_with(|| (selection.clone(), 0));
    }
    let mut values: Vec<FacetValue> = counts
        .into_values()
        .map(|(value, count)| FacetValue { value, count })
        .collect();
    sort_facet_values(&mut values);
    values
}

/// Fixed-order facet (loading modes / compatibility bands): include values
/// with count > 0 or currently selected, in the given canonical order.
fn fixed_order_facet(
    order: &[&str],
    counts: &HashMap<String, i64>,
    selections: &[String],
) -> Vec<FacetValue> {
    order
        .iter()
        .filter_map(|value| {
            let count = counts.get(*value).copied().unwrap_or(0);
            (count > 0 || selections.iter().any(|selection| selection == value)).then(|| {
                FacetValue {
                    value: (*value).to_string(),
                    count,
                }
            })
        })
        .collect()
}

fn truncate_title_facet(values: Vec<FacetValue>, selections: &[String]) -> Vec<FacetValue> {
    if values.len() <= CATALOG_TITLE_FACET_CAP {
        return values;
    }
    let pinned: Vec<FacetValue> = values
        .iter()
        .skip(CATALOG_TITLE_FACET_CAP)
        .filter(|facet| {
            let value = facet.value.to_lowercase();
            selections
                .iter()
                .any(|selection| selection.to_lowercase() == value)
        })
        .cloned()
        .collect();
    let mut truncated: Vec<FacetValue> = values;
    truncated.truncate(CATALOG_TITLE_FACET_CAP - pinned.len().min(CATALOG_TITLE_FACET_CAP));
    truncated.extend(pinned);
    truncated
}

/// One trainer-source group keyed by `(tap_id, lowercased game name)`.
struct SourceGroup {
    tap_url: String,
    tap_local_path: String,
    /// First-seen casing, used for synthesized source-only entries.
    game_name: String,
    sources: Vec<CatalogSource>,
}

/// Synthesize a catalog entry for a source group with no matching profile
/// row, so source-only games still surface in the catalog. `manifest_path`
/// stays empty (nothing to import); rating and loading mode bucket to
/// "unknown".
fn source_only_entry(tap_id: String, game_key: &str, group: SourceGroup) -> CatalogRowInput {
    CatalogRowInput {
        tap_id,
        entry: CatalogEntry {
            id: None,
            tap_url: group.tap_url,
            tap_local_path: group.tap_local_path,
            relative_path: format!("trainer-sources/{game_key}"),
            manifest_path: String::new(),
            game_name: Some(group.game_name),
            game_version: None,
            trainer_name: None,
            trainer_version: None,
            proton_version: None,
            compatibility_rating: None,
            author: None,
            description: None,
            platform_tags: None,
            trainer_loading_mode: None,
            schema_version: 0,
            sources: group.sources,
        },
    }
}

/// One pure pass: attach sources, synthesize source-only entries, filter,
/// facet-count, rank, and paginate.
pub fn build_catalog_page(
    rows: Vec<CatalogRowInput>,
    sources: Vec<CatalogSourceInput>,
    query: &CatalogQuery,
    degraded: bool,
) -> CatalogPage {
    let normalized = query.query.as_deref().unwrap_or("").trim().to_lowercase();
    let text_query = (!normalized.is_empty()).then_some(normalized.as_str());

    let mut source_groups: HashMap<(String, String), SourceGroup> = HashMap::new();
    for input in sources {
        let CatalogSourceInput {
            tap_id,
            tap_url,
            tap_local_path,
            game_name,
            source,
        } = input;
        source_groups
            .entry((tap_id, game_name.to_lowercase()))
            .or_insert_with(|| SourceGroup {
                tap_url,
                tap_local_path,
                game_name,
                sources: Vec::new(),
            })
            .sources
            .push(source);
    }

    let mut rows: Vec<CatalogRowInput> = rows
        .into_iter()
        .map(|mut row| {
            if let Some(game_name) = row.entry.game_name.as_deref() {
                row.entry.sources = source_groups
                    .get(&(row.tap_id.clone(), game_name.to_lowercase()))
                    .map(|group| group.sources.clone())
                    .unwrap_or_default();
            }
            row
        })
        .collect();

    let covered: HashSet<(String, String)> = rows
        .iter()
        .filter_map(|row| {
            row.entry
                .game_name
                .as_deref()
                .map(|name| (row.tap_id.clone(), name.to_lowercase()))
        })
        .collect();
    let mut orphan_groups: Vec<((String, String), SourceGroup)> = source_groups
        .into_iter()
        .filter(|(key, _)| !covered.contains(key))
        .collect();
    orphan_groups.sort_by(|a, b| a.0.cmp(&b.0));
    rows.extend(
        orphan_groups
            .into_iter()
            .map(|((tap_id, game_key), group)| source_only_entry(tap_id, &game_key, group)),
    );

    let tap_count = rows
        .iter()
        .map(|row| row.entry.tap_url.as_str())
        .collect::<HashSet<_>>()
        .len() as i64;

    let text_matched: Vec<&CatalogRowInput> = rows
        .iter()
        .filter(|row| text_query.is_none_or(|q| matches_text(&row.entry, q)))
        .collect();

    let mut title_counts: HashMap<String, (String, i64)> = HashMap::new();
    let mut mode_counts: HashMap<String, i64> = HashMap::new();
    let mut band_counts: HashMap<String, i64> = HashMap::new();
    let mut tap_counts: HashMap<String, i64> = HashMap::new();

    for row in &text_matched {
        let entry = &row.entry;
        let passes_titles = matches_game_titles(entry, &query.game_titles);
        let passes_modes = matches_value(mode(entry), &query.loading_modes);
        let passes_bands = matches_value(band(entry), &query.compatibility_bands);
        let passes_taps = matches_value(&entry.tap_url, &query.tap_urls);

        if passes_modes && passes_bands && passes_taps {
            if let Some(game_name) = entry.game_name.as_deref() {
                title_counts
                    .entry(game_name.to_lowercase())
                    .or_insert_with(|| (game_name.to_string(), 0))
                    .1 += 1;
            }
        }
        if passes_titles && passes_bands && passes_taps {
            *mode_counts.entry(mode(entry).to_string()).or_insert(0) += 1;
        }
        if passes_titles && passes_modes && passes_taps {
            *band_counts.entry(band(entry).to_string()).or_insert(0) += 1;
        }
        if passes_titles && passes_modes && passes_bands {
            *tap_counts.entry(entry.tap_url.clone()).or_insert(0) += 1;
        }
    }

    let facets = CatalogFacets {
        game_titles: truncate_title_facet(
            ranked_title_facet(title_counts, &query.game_titles),
            &query.game_titles,
        ),
        loading_modes: fixed_order_facet(&LOADING_MODE_ORDER, &mode_counts, &query.loading_modes),
        compatibility_bands: fixed_order_facet(
            &COMPATIBILITY_BAND_ORDER,
            &band_counts,
            &query.compatibility_bands,
        ),
        taps: ranked_facet(tap_counts, &query.tap_urls),
    };

    let mut matched: Vec<&CatalogRowInput> = text_matched
        .into_iter()
        .filter(|row| {
            let entry = &row.entry;
            matches_game_titles(entry, &query.game_titles)
                && matches_value(mode(entry), &query.loading_modes)
                && matches_value(band(entry), &query.compatibility_bands)
                && matches_value(&entry.tap_url, &query.tap_urls)
        })
        .collect();
    let total_count = matched.len() as i64;

    matched.sort_by_cached_key(|row| catalog_sort_key(&row.entry, text_query));

    let offset = query.offset.unwrap_or(0) as usize;
    let limit = query
        .limit
        .unwrap_or(CATALOG_DEFAULT_LIMIT)
        .min(CATALOG_MAX_LIMIT) as usize;
    let entries: Vec<CatalogEntry> = matched
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|row| row.entry.clone())
        .collect();

    CatalogPage {
        entries,
        facets,
        total_count,
        tap_count,
        degraded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RowSpec<'a> {
        tap: &'a str,
        game: Option<&'a str>,
        trainer: Option<&'a str>,
        rating: Option<&'a str>,
        mode: Option<&'a str>,
        author: Option<&'a str>,
        path: &'a str,
    }

    impl Default for RowSpec<'_> {
        fn default() -> Self {
            Self {
                tap: "https://example.com/tap-a.git",
                game: Some("Elden Ring"),
                trainer: None,
                rating: Some("working"),
                mode: Some("source_directory"),
                author: None,
                path: "a.json",
            }
        }
    }

    fn row(spec: RowSpec<'_>) -> CatalogRowInput {
        CatalogRowInput {
            tap_id: spec.tap.to_string(),
            entry: CatalogEntry {
                id: None,
                tap_url: spec.tap.to_string(),
                tap_local_path: "/tmp/tap".to_string(),
                relative_path: spec.path.to_string(),
                manifest_path: spec.path.to_string(),
                game_name: spec.game.map(str::to_string),
                game_version: None,
                trainer_name: spec.trainer.map(str::to_string),
                trainer_version: None,
                proton_version: None,
                compatibility_rating: spec.rating.map(str::to_string),
                author: spec.author.map(str::to_string),
                description: None,
                platform_tags: None,
                trainer_loading_mode: spec.mode.map(str::to_string),
                schema_version: 1,
                sources: Vec::new(),
            },
        }
    }

    fn source(tap_id: &str, game_name: &str, source_name: &str) -> CatalogSourceInput {
        CatalogSourceInput {
            tap_id: tap_id.to_string(),
            tap_url: tap_id.to_string(),
            tap_local_path: "/tmp/tap".to_string(),
            game_name: game_name.to_string(),
            source: CatalogSource {
                source_name: source_name.to_string(),
                source_url: "https://example.com/trainer.exe".to_string(),
                sha256: None,
                trainer_version: None,
                game_version: None,
                notes: None,
            },
        }
    }

    fn facet_count(values: &[FacetValue], value: &str) -> Option<i64> {
        values
            .iter()
            .find(|facet| facet.value == value)
            .map(|facet| facet.count)
    }

    #[test]
    fn facet_counts_use_other_dimension_semantics() {
        let rows = vec![
            row(RowSpec {
                rating: Some("working"),
                path: "a.json",
                ..RowSpec::default()
            }),
            row(RowSpec {
                game: Some("Sekiro"),
                rating: Some("platinum"),
                path: "b.json",
                ..RowSpec::default()
            }),
        ];

        // Selecting a band never zeroes sibling bands.
        let query = CatalogQuery {
            compatibility_bands: vec!["working".to_string()],
            ..CatalogQuery::default()
        };
        let page = build_catalog_page(rows.clone(), Vec::new(), &query, false);
        assert_eq!(page.total_count, 1);
        assert_eq!(
            facet_count(&page.facets.compatibility_bands, "platinum"),
            Some(1),
            "sibling band counts must ignore the band selection itself"
        );
        assert_eq!(
            facet_count(&page.facets.compatibility_bands, "working"),
            Some(1)
        );
        // Non-band facets apply the band filter.
        assert_eq!(facet_count(&page.facets.game_titles, "Sekiro"), None);

        // Title counts shrink under a text query.
        let query = CatalogQuery {
            query: Some("sekiro".to_string()),
            ..CatalogQuery::default()
        };
        let page = build_catalog_page(rows, Vec::new(), &query, false);
        assert_eq!(facet_count(&page.facets.game_titles, "Sekiro"), Some(1));
        assert_eq!(facet_count(&page.facets.game_titles, "Elden Ring"), None);
    }

    #[test]
    fn selected_facet_values_are_pinned_at_zero_count() {
        let rows = vec![row(RowSpec::default())];
        let query = CatalogQuery {
            game_titles: vec!["Ghost Game".to_string()],
            compatibility_bands: vec!["broken".to_string()],
            ..CatalogQuery::default()
        };
        let page = build_catalog_page(rows, Vec::new(), &query, false);

        assert_eq!(page.total_count, 0);
        assert_eq!(facet_count(&page.facets.game_titles, "Ghost Game"), Some(0));
        assert_eq!(
            facet_count(&page.facets.compatibility_bands, "broken"),
            Some(0)
        );
    }

    #[test]
    fn null_rating_and_loading_mode_bucket_to_unknown() {
        let rows = vec![
            row(RowSpec {
                rating: None,
                mode: None,
                path: "a.json",
                ..RowSpec::default()
            }),
            row(RowSpec {
                rating: Some("garbage"),
                mode: Some("garbage"),
                path: "b.json",
                ..RowSpec::default()
            }),
        ];
        let page = build_catalog_page(rows.clone(), Vec::new(), &CatalogQuery::default(), false);
        assert_eq!(
            facet_count(&page.facets.compatibility_bands, "unknown"),
            Some(2)
        );
        assert_eq!(facet_count(&page.facets.loading_modes, "unknown"), Some(2));

        // Selecting "unknown" matches both null and garbage values.
        let query = CatalogQuery {
            loading_modes: vec!["unknown".to_string()],
            compatibility_bands: vec!["unknown".to_string()],
            ..CatalogQuery::default()
        };
        let page = build_catalog_page(rows, Vec::new(), &query, false);
        assert_eq!(page.total_count, 2);
    }

    #[test]
    fn null_game_name_rows_excluded_from_title_facet_but_present_in_results() {
        let rows = vec![
            row(RowSpec {
                game: None,
                trainer: Some("Nameless Trainer"),
                path: "n.json",
                ..RowSpec::default()
            }),
            row(RowSpec::default()),
        ];
        let page = build_catalog_page(rows.clone(), Vec::new(), &CatalogQuery::default(), false);
        assert_eq!(page.total_count, 2);
        assert_eq!(page.facets.game_titles.len(), 1);
        assert_eq!(facet_count(&page.facets.game_titles, "Elden Ring"), Some(1));

        // A non-empty title selection can never match a None game name.
        let query = CatalogQuery {
            game_titles: vec!["Elden Ring".to_string()],
            ..CatalogQuery::default()
        };
        let page = build_catalog_page(rows, Vec::new(), &query, false);
        assert_eq!(page.total_count, 1);
        assert_eq!(page.entries[0].game_name.as_deref(), Some("Elden Ring"));
    }

    #[test]
    fn limit_defaults_to_50_caps_at_200_and_offset_pages() {
        let rows: Vec<CatalogRowInput> = (0..250)
            .map(|i| {
                row(RowSpec {
                    game: Some(Box::leak(format!("Game {i:03}").into_boxed_str())),
                    path: Box::leak(format!("{i:03}.json").into_boxed_str()),
                    ..RowSpec::default()
                })
            })
            .collect();

        let page = build_catalog_page(rows.clone(), Vec::new(), &CatalogQuery::default(), false);
        assert_eq!(page.entries.len(), 50, "default limit is 50");
        assert_eq!(page.total_count, 250);

        let query = CatalogQuery {
            limit: Some(500),
            ..CatalogQuery::default()
        };
        let page = build_catalog_page(rows.clone(), Vec::new(), &query, false);
        assert_eq!(page.entries.len(), 200, "limit caps at 200");

        let query = CatalogQuery {
            limit: Some(2),
            offset: Some(2),
            ..CatalogQuery::default()
        };
        let page = build_catalog_page(rows, Vec::new(), &query, false);
        assert_eq!(page.entries.len(), 2);
        assert_eq!(page.entries[0].game_name.as_deref(), Some("Game 002"));
        assert_eq!(page.entries[1].game_name.as_deref(), Some("Game 003"));
    }

    #[test]
    fn secondary_haystack_matches_without_affecting_tier() {
        let rows = vec![
            row(RowSpec {
                game: Some("Sekiro"),
                author: Some("FromFan"),
                rating: Some("platinum"),
                path: "author-only.json",
                ..RowSpec::default()
            }),
            row(RowSpec {
                game: Some("Fromville"),
                rating: Some("broken"),
                path: "name-match.json",
                ..RowSpec::default()
            }),
        ];
        let query = CatalogQuery {
            query: Some("from".to_string()),
            ..CatalogQuery::default()
        };
        let page = build_catalog_page(rows, Vec::new(), &query, false);

        assert_eq!(
            page.total_count, 2,
            "author-only match is included and counted"
        );
        assert_eq!(
            page.entries[0].game_name.as_deref(),
            Some("Fromville"),
            "any name match outranks a secondary-field match despite worse rating"
        );
        assert_eq!(page.entries[1].game_name.as_deref(), Some("Sekiro"));
    }

    #[test]
    fn text_query_matches_source_provider_name_and_notes() {
        let rows = vec![
            row(RowSpec::default()),
            row(RowSpec {
                game: Some("Sekiro"),
                path: "sekiro.json",
                ..RowSpec::default()
            }),
        ];
        let mut fling = source("https://example.com/tap-a.git", "Elden Ring", "FLiNG");
        fling.source.notes = Some("Requires table version 7".to_string());
        let sources = vec![fling];

        let page = build_catalog_page(
            rows.clone(),
            sources.clone(),
            &CatalogQuery {
                query: Some("fling".to_string()),
                ..CatalogQuery::default()
            },
            false,
        );
        assert_eq!(page.total_count, 1, "provider name must match");
        assert_eq!(page.entries[0].game_name.as_deref(), Some("Elden Ring"));

        let page = build_catalog_page(
            rows,
            sources,
            &CatalogQuery {
                query: Some("table version".to_string()),
                ..CatalogQuery::default()
            },
            false,
        );
        assert_eq!(page.total_count, 1, "note substring must match");
        assert_eq!(page.entries[0].game_name.as_deref(), Some("Elden Ring"));
    }

    #[test]
    fn text_query_matches_version_fields_and_manifest_path() {
        let mut versioned = row(RowSpec {
            path: "versioned.json",
            ..RowSpec::default()
        });
        versioned.entry.game_version = Some("1.10.1".to_string());
        versioned.entry.trainer_version = Some("v42-beta".to_string());
        versioned.entry.proton_version = Some("GE-Proton9-21".to_string());
        let other = row(RowSpec {
            game: Some("Sekiro"),
            path: "other.json",
            ..RowSpec::default()
        });

        for query in ["1.10.1", "v42-beta", "ge-proton9"] {
            let page = build_catalog_page(
                vec![versioned.clone(), other.clone()],
                Vec::new(),
                &CatalogQuery {
                    query: Some(query.to_string()),
                    ..CatalogQuery::default()
                },
                false,
            );
            assert_eq!(page.total_count, 1, "version query {query:?} must match");
            assert_eq!(page.entries[0].game_name.as_deref(), Some("Elden Ring"));
        }

        let page = build_catalog_page(
            vec![versioned, other],
            Vec::new(),
            &CatalogQuery {
                query: Some("versioned.json".to_string()),
                ..CatalogQuery::default()
            },
            false,
        );
        assert_eq!(page.total_count, 1, "manifest path must match");
    }

    #[test]
    fn game_title_facet_groups_and_matches_case_insensitively() {
        let rows = vec![
            row(RowSpec {
                game: Some("ELDEN RING"),
                path: "upper.json",
                ..RowSpec::default()
            }),
            row(RowSpec {
                game: Some("elden ring"),
                path: "lower.json",
                ..RowSpec::default()
            }),
        ];

        let page = build_catalog_page(rows.clone(), Vec::new(), &CatalogQuery::default(), false);
        assert_eq!(
            page.facets.game_titles.len(),
            1,
            "case variants collapse into one facet option"
        );
        assert_eq!(page.facets.game_titles[0].value, "ELDEN RING");
        assert_eq!(page.facets.game_titles[0].count, 2);

        let query = CatalogQuery {
            game_titles: vec!["ELDEN RING".to_string()],
            ..CatalogQuery::default()
        };
        let page = build_catalog_page(rows, Vec::new(), &query, false);
        assert_eq!(
            page.total_count, 2,
            "selecting the facet matches both casings"
        );
    }

    #[test]
    fn sources_group_by_tap_id_and_lowercased_game_name() {
        let rows = vec![
            row(RowSpec {
                tap: "tap-a",
                game: Some("ELDEN RING"),
                path: "a.json",
                ..RowSpec::default()
            }),
            row(RowSpec {
                tap: "tap-b",
                game: Some("Elden Ring"),
                path: "b.json",
                ..RowSpec::default()
            }),
        ];
        let sources = vec![source("tap-a", "elden ring", "Tap A Source")];
        let page = build_catalog_page(rows, sources, &CatalogQuery::default(), false);

        let tap_a_entry = page
            .entries
            .iter()
            .find(|entry| entry.tap_url == "tap-a")
            .unwrap();
        let tap_b_entry = page
            .entries
            .iter()
            .find(|entry| entry.tap_url == "tap-b")
            .unwrap();
        assert_eq!(
            tap_a_entry.sources.len(),
            1,
            "case-differing manifest game_name still attaches"
        );
        assert_eq!(tap_a_entry.sources[0].source_name, "Tap A Source");
        assert!(
            tap_b_entry.sources.is_empty(),
            "same game on a different tap must not cross-attach"
        );
    }

    #[test]
    fn entries_without_source_group_get_empty_sources() {
        let rows = vec![
            row(RowSpec::default()),
            row(RowSpec {
                game: None,
                path: "none.json",
                ..RowSpec::default()
            }),
        ];
        let sources = vec![source(
            "https://example.com/tap-a.git",
            "Some Other Game",
            "Unrelated",
        )];
        let page = build_catalog_page(rows, sources, &CatalogQuery::default(), false);
        assert!(page
            .entries
            .iter()
            .filter(|entry| !entry.manifest_path.is_empty())
            .all(|entry| entry.sources.is_empty()));
    }

    #[test]
    fn source_group_without_profile_row_synthesizes_source_only_entry() {
        let rows = vec![row(RowSpec::default())];
        let sources = vec![source(
            "https://example.com/tap-a.git",
            "Source Only Game",
            "FLiNG",
        )];
        let page = build_catalog_page(rows, sources, &CatalogQuery::default(), false);

        assert_eq!(page.total_count, 2);
        let synthesized = page
            .entries
            .iter()
            .find(|entry| entry.game_name.as_deref() == Some("Source Only Game"))
            .expect("source-only entry must appear in the catalog");
        assert!(synthesized.manifest_path.is_empty(), "nothing to import");
        assert_eq!(synthesized.id, None);
        assert_eq!(synthesized.trainer_name, None);
        assert_eq!(synthesized.compatibility_rating, None);
        assert_eq!(synthesized.trainer_loading_mode, None);
        assert_eq!(synthesized.tap_url, "https://example.com/tap-a.git");
        assert_eq!(synthesized.sources.len(), 1);
        assert_eq!(synthesized.sources[0].source_name, "FLiNG");

        // Participates in facets: bucketed unknown, title, and tap counts.
        assert_eq!(
            facet_count(&page.facets.compatibility_bands, "unknown"),
            Some(1)
        );
        assert_eq!(facet_count(&page.facets.loading_modes, "unknown"), Some(1));
        assert_eq!(
            facet_count(&page.facets.game_titles, "Source Only Game"),
            Some(1)
        );

        // Participates in text match via the game name.
        let query = CatalogQuery {
            query: Some("source only".to_string()),
            ..CatalogQuery::default()
        };
        let page = build_catalog_page(
            vec![row(RowSpec::default())],
            vec![source(
                "https://example.com/tap-a.git",
                "Source Only Game",
                "FLiNG",
            )],
            &query,
            false,
        );
        assert_eq!(page.total_count, 1);
        assert_eq!(
            page.entries[0].game_name.as_deref(),
            Some("Source Only Game")
        );
    }

    #[test]
    fn source_only_taps_count_toward_tap_count() {
        let rows = vec![row(RowSpec::default())];
        let sources = vec![source("tap-sources-only", "Lonely Game", "FLiNG")];
        let page = build_catalog_page(rows, sources, &CatalogQuery::default(), false);
        assert_eq!(page.tap_count, 2);
    }

    #[test]
    fn total_count_is_pre_pagination() {
        let rows: Vec<CatalogRowInput> = (0..5)
            .map(|i| {
                row(RowSpec {
                    path: Box::leak(format!("{i}.json").into_boxed_str()),
                    ..RowSpec::default()
                })
            })
            .collect();
        let query = CatalogQuery {
            limit: Some(2),
            ..CatalogQuery::default()
        };
        let page = build_catalog_page(rows, Vec::new(), &query, false);
        assert_eq!(page.entries.len(), 2);
        assert_eq!(page.total_count, 5);
    }

    #[test]
    fn tap_count_counts_distinct_taps_pre_filter() {
        let rows = vec![
            row(RowSpec {
                tap: "tap-a",
                ..RowSpec::default()
            }),
            row(RowSpec {
                tap: "tap-b",
                game: Some("Sekiro"),
                path: "b.json",
                ..RowSpec::default()
            }),
        ];
        let query = CatalogQuery {
            query: Some("no-match-at-all".to_string()),
            ..CatalogQuery::default()
        };
        let page = build_catalog_page(rows, Vec::new(), &query, false);
        assert_eq!(page.total_count, 0);
        assert_eq!(page.tap_count, 2, "tap count covers the unfiltered corpus");
    }

    #[test]
    fn empty_corpus_returns_empty_non_degraded_page() {
        let page = build_catalog_page(Vec::new(), Vec::new(), &CatalogQuery::default(), false);
        assert!(page.entries.is_empty());
        assert_eq!(page.facets, CatalogFacets::default());
        assert_eq!(page.total_count, 0);
        assert_eq!(page.tap_count, 0);
        assert!(!page.degraded);
    }

    #[test]
    fn game_title_facet_caps_at_100_values_keeping_pinned() {
        let rows: Vec<CatalogRowInput> = (0..150)
            .map(|i| {
                row(RowSpec {
                    game: Some(Box::leak(format!("Title {i:03}").into_boxed_str())),
                    path: Box::leak(format!("{i:03}.json").into_boxed_str()),
                    ..RowSpec::default()
                })
            })
            .collect();

        // "Title 149" sorts past the cap (all counts equal, value ASC).
        let query = CatalogQuery {
            game_titles: vec!["Title 149".to_string()],
            ..CatalogQuery::default()
        };
        let page = build_catalog_page(rows, Vec::new(), &query, false);

        assert_eq!(page.facets.game_titles.len(), CATALOG_TITLE_FACET_CAP);
        assert_eq!(
            facet_count(&page.facets.game_titles, "Title 149"),
            Some(1),
            "the selected value must survive truncation"
        );
    }
}
