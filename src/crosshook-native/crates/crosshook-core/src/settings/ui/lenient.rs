use serde::{Deserialize, Deserializer};
use toml::{Table, Value};

use super::{ControllerMode, LibraryViewMode, UiDensity, UiRenderer, UiSettings, UI_SCALE_DEFAULT};

impl<'de> Deserialize<'de> for UiSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let Value::Table(mut table) = value else {
            warn_invalid("ui", "not_a_table");
            return Ok(Self::default());
        };

        Ok(Self {
            library_view_mode: take_enum(&mut table, "library_view_mode", LibraryViewMode::parse),
            window_width: take_clamped_u32(
                &mut table,
                "window_width",
                super::UI_WINDOW_WIDTH_MIN,
                super::UI_WINDOW_WIDTH_MAX,
            ),
            window_height: take_clamped_u32(
                &mut table,
                "window_height",
                super::UI_WINDOW_HEIGHT_MIN,
                super::UI_WINDOW_HEIGHT_MAX,
            ),
            ui_scale: take_scale(&mut table).unwrap_or(UI_SCALE_DEFAULT),
            density: take_enum(&mut table, "density", UiDensity::parse).unwrap_or_default(),
            renderer: take_enum(&mut table, "renderer", UiRenderer::parse).unwrap_or_default(),
            controller_mode: take_enum(&mut table, "controller_mode", ControllerMode::parse)
                .unwrap_or_default(),
            sidebar_collapsed: take_bool(&mut table, "sidebar_collapsed").unwrap_or_default(),
            inspector_width: take_clamped_u32(
                &mut table,
                "inspector_width",
                super::UI_INSPECTOR_WIDTH_MIN,
                super::UI_INSPECTOR_WIDTH_MAX,
            )
            .unwrap_or(super::UI_INSPECTOR_WIDTH_DEFAULT),
            console_drawer_height: take_clamped_u32(
                &mut table,
                "console_drawer_height",
                super::UI_CONSOLE_DRAWER_HEIGHT_MIN,
                super::UI_CONSOLE_DRAWER_HEIGHT_MAX,
            )
            .unwrap_or(super::UI_CONSOLE_DRAWER_HEIGHT_DEFAULT),
            palette_recent: take_palette(&mut table),
            extra: table,
        })
    }
}

fn warn_invalid(key: &'static str, reason: &'static str) {
    tracing::warn!(key, reason, "ignoring invalid [ui] setting");
}

fn take_enum<T>(
    table: &mut Table,
    key: &'static str,
    parse: impl FnOnce(&str) -> Option<T>,
) -> Option<T> {
    let value = table.remove(key)?;
    let Value::String(raw) = value else {
        warn_invalid(key, "invalid_type");
        return None;
    };
    let parsed = parse(&raw);
    if parsed.is_none() {
        warn_invalid(key, "invalid_value");
    }
    parsed
}

fn take_clamped_u32(table: &mut Table, key: &'static str, min: u32, max: u32) -> Option<u32> {
    let value = table.remove(key)?;
    let Value::Integer(raw) = value else {
        warn_invalid(key, "invalid_type");
        return None;
    };
    let clamped = raw.clamp(i64::from(min), i64::from(max));
    if clamped != raw {
        warn_invalid(key, "out_of_range");
    }
    u32::try_from(clamped).ok()
}

fn take_bool(table: &mut Table, key: &'static str) -> Option<bool> {
    let value = table.remove(key)?;
    let Value::Boolean(value) = value else {
        warn_invalid(key, "invalid_type");
        return None;
    };
    Some(value)
}

fn take_scale(table: &mut Table) -> Option<f64> {
    let value = table.remove("ui_scale")?;
    let raw = match value {
        Value::Float(value) => value,
        Value::Integer(value) => value as f64,
        _ => {
            warn_invalid("ui_scale", "invalid_type");
            return None;
        }
    };
    if !raw.is_finite() {
        warn_invalid("ui_scale", "non_finite");
        return Some(UI_SCALE_DEFAULT);
    }
    let clamped = raw.clamp(super::UI_SCALE_MIN, super::UI_SCALE_MAX);
    if clamped != raw {
        warn_invalid("ui_scale", "out_of_range");
    }
    Some(clamped)
}

fn take_palette(table: &mut Table) -> Vec<String> {
    let Some(value) = table.remove("palette_recent") else {
        return Vec::new();
    };
    let Value::Array(values) = value else {
        warn_invalid("palette_recent", "invalid_type");
        return Vec::new();
    };

    let mut non_string = 0;
    let mut list = Vec::with_capacity(values.len());
    for value in values {
        match value {
            Value::String(id) => list.push(id),
            _ => non_string += 1,
        }
    }
    let (dropped_invalid, truncated) = super::palette::sanitize_palette_recent(&mut list);
    let total_dropped = non_string + dropped_invalid;
    if total_dropped != 0 || truncated != 0 {
        tracing::warn!(
            key = "palette_recent",
            reason = "invalid_entries",
            dropped = total_dropped,
            truncated,
            "sanitizing invalid [ui] palette entries"
        );
    }
    list
}
