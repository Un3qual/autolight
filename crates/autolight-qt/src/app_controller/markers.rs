use autolight_core::project::{JsonObject, Marker};
use serde_json::{json, Value};

const DEFAULT_MARKER_COLOR: &str = "cyan";
const MARKER_COLOR_OPTIONS: &[(&str, &str, &str)] = &[
    ("cyan", "Cyan", "#67e8f9"),
    ("green", "Green", "#a7f3d0"),
    ("amber", "Amber", "#fbbf24"),
    ("violet", "Violet", "#c4b5fd"),
    ("rose", "Rose", "#fda4af"),
    ("blue", "Blue", "#93c5fd"),
];

pub(super) fn finite_non_negative(value: f64) -> f64 {
    if value.is_finite() && value >= 0.0 {
        value
    } else {
        0.0
    }
}

pub(super) fn finite_duration(value: f64) -> Option<f64> {
    (value.is_finite() && value >= 0.0).then_some(value)
}

pub(super) fn marker_end_seconds(marker: &Marker) -> Option<f64> {
    let timestamp = finite_duration(marker.timestamp)?;
    let duration = marker.duration.and_then(finite_duration).unwrap_or(0.0);
    Some(timestamp + duration)
}

pub(super) fn marker_color_options_json() -> String {
    json_string(
        &MARKER_COLOR_OPTIONS
            .iter()
            .map(|(key, label, color)| {
                json!({
                    "key": key,
                    "label": label,
                    "color": color,
                })
            })
            .collect::<Vec<_>>(),
    )
}

pub(super) fn marker_color_key(marker: &Marker) -> &'static str {
    let color = marker
        .metadata
        .get("color")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_MARKER_COLOR);
    MARKER_COLOR_OPTIONS
        .iter()
        .find_map(|(key, _, _)| (*key == color).then_some(*key))
        .unwrap_or(DEFAULT_MARKER_COLOR)
}

pub(super) fn marker_display_color_for_key(color_key: &str) -> &'static str {
    MARKER_COLOR_OPTIONS
        .iter()
        .find_map(|(key, _, color)| (*key == color_key).then_some(*color))
        .unwrap_or("#67e8f9")
}

pub(super) fn is_timing_snap_category(category: &str) -> bool {
    let category = category.trim();
    category.eq_ignore_ascii_case("timing")
        || category.eq_ignore_ascii_case("beat")
        || category.eq_ignore_ascii_case("onset")
}

pub(super) fn json_string(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "[]".to_string())
}

pub(super) fn json_object(values: impl IntoIterator<Item = (&'static str, Value)>) -> JsonObject {
    values
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

pub(super) fn round6(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}
