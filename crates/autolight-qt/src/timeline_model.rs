use std::collections::{BTreeMap, BTreeSet};

use autolight_core::cache::artifact_kinds_for_track;
use autolight_core::graph::{
    default_expanded_track_ids, project_tree, source_track_id_for_context,
};
use autolight_core::project::{
    AudioAsset, CacheEntry, JobRun, JsonObject, Marker, ProjectDocument, ResultState, Track,
    TrackType,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const RUST_DEMO_PROJECT_NAME: &str = "Autolight Rust Demo";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineRow {
    pub track_id: String,
    pub name: String,
    pub track_type: String,
    pub result_state: String,
    pub marker_count: usize,
    pub marker_spans: Vec<MarkerSpan>,
    pub error: String,
    pub active_job_id: String,
    pub job_state: String,
    pub job_progress: f64,
    pub waveform_samples: Vec<Value>,
    pub cache_ref_count: usize,
    pub artifact_kinds: String,
    pub waveform_duration_seconds: f64,
    pub editable: bool,
    pub visible_waveform_samples: Vec<Value>,
    pub waveform_level_bucket_count: usize,
    pub parent_track_id: String,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
    pub child_count: usize,
    pub visible_child_state_summary: String,
    pub tree_error: String,
    pub visible_energy_samples: Vec<Value>,
    pub visible_harmonic_color_samples: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkerSpan {
    pub id: String,
    pub timestamp: f64,
    pub duration: f64,
    pub label: String,
    pub category: String,
    pub color: String,
    pub selected: bool,
}

pub fn rust_demo_project() -> ProjectDocument {
    let mut project = ProjectDocument::new("project_rust_demo", RUST_DEMO_PROJECT_NAME);
    project.audio_assets.push(AudioAsset {
        id: "asset_demo".to_string(),
        path: "/fixtures/audio/rust-demo.wav".to_string(),
        duration: 2.0,
        sample_rate: 44_100,
        channels: 2,
        fingerprint: "rust-demo-fingerprint".to_string(),
        import_status: "online".to_string(),
        relink_hint: String::new(),
    });
    project.cache_entries.extend([
        cache_entry(
            "cache_waveform",
            "waveform",
            "cache/waveform/rust-demo.json",
        ),
        cache_entry("cache_drums", "stem", "cache/stem/drums.wav"),
        cache_entry("cache_energy", "energy", "cache/energy/drums.json"),
    ]);
    project.tracks.extend([
        source_track(),
        generated_track(
            "track_beats",
            "Beat Markers",
            "track_source",
            "markers.fixed_interval",
            "markers.v1",
            "dep_beats",
            ResultState::Complete,
            Vec::new(),
            JsonObject::new(),
        ),
        editable_track(),
        generated_track(
            "track_waveform",
            "Waveform Summary",
            "track_source",
            "waveform.summary",
            "artifact.waveform.v1",
            "dep_waveform",
            ResultState::Complete,
            vec!["cache_waveform".to_string()],
            json_object([(
                "visible_waveform",
                json!({
                    "duration_seconds": 2.0,
                    "level_bucket_count": 64,
                    "samples": [
                        {"min": -0.1, "max": 0.2},
                        {"min": -0.3, "max": 0.4}
                    ]
                }),
            )]),
        ),
        generated_track(
            "track_drums",
            "Drums Stem",
            "track_source",
            "audio.drums_stand_in",
            "artifact.audio.v1",
            "dep_drums",
            ResultState::Complete,
            vec!["cache_drums".to_string()],
            json_object([("stem", json!("drums"))]),
        ),
        generated_track(
            "track_drum_energy",
            "Drum Energy",
            "track_drums",
            "music.energy_profile",
            "artifact.energy.v1",
            "dep_energy",
            ResultState::Pending,
            vec!["cache_energy".to_string()],
            json_object([(
                "visible_energy",
                json!([
                    {"timestamp": 0.0, "value": 0.2},
                    {"timestamp": 0.5, "value": 0.8}
                ]),
            )]),
        ),
    ]);
    project.markers.extend([
        marker(
            "marker_demo_1",
            "track_beats",
            0.0,
            None,
            "Beat",
            "timing",
            "cyan",
            Vec::new(),
        ),
        marker(
            "marker_demo_2",
            "track_beats",
            0.5,
            None,
            "Beat",
            "timing",
            "green",
            Vec::new(),
        ),
        marker(
            "marker_demo_3",
            "track_beats",
            1.0,
            None,
            "Beat",
            "timing",
            "blue",
            Vec::new(),
        ),
        marker(
            "marker_edit_1",
            "track_edit",
            0.0,
            Some(0.25),
            "Cue",
            "cue",
            "amber",
            vec!["marker_demo_1".to_string()],
        ),
        marker(
            "marker_edit_2",
            "track_edit",
            0.5,
            Some(0.25),
            "Cue",
            "cue",
            "violet",
            vec!["marker_demo_2".to_string()],
        ),
    ]);
    project.job_runs.push(JobRun {
        id: "job_drum_energy".to_string(),
        track_id: "track_drum_energy".to_string(),
        transform_id: "music.energy_profile".to_string(),
        parameters_hash: "dep_energy".to_string(),
        parameters: JsonObject::new(),
        state: ResultState::Pending,
        progress: 0.0,
        started_at: String::new(),
        completed_at: String::new(),
        error: String::new(),
        produced_cache_refs: Vec::new(),
    });
    project
}

pub fn timeline_rows_for_project(project: &ProjectDocument) -> Vec<TimelineRow> {
    let expanded = default_expanded_track_ids(project);
    timeline_rows_for_project_with_state(project, &expanded, &BTreeSet::new())
}

pub fn timeline_rows_for_project_with_state(
    project: &ProjectDocument,
    expanded: &BTreeSet<String>,
    selected_marker_ids: &BTreeSet<String>,
) -> Vec<TimelineRow> {
    let tree_rows = project_tree(project, expanded);
    let tracks_by_id: BTreeMap<&str, &Track> = project
        .tracks
        .iter()
        .map(|track| (track.id.as_str(), track))
        .collect();

    tree_rows
        .into_iter()
        .filter_map(|tree_row| {
            let track = tracks_by_id.get(tree_row.track_id.as_str())?;
            let latest_job = project
                .job_runs
                .iter()
                .rev()
                .find(|run| run.track_id == track.id);
            Some(TimelineRow {
                track_id: track.id.clone(),
                name: track.name.clone(),
                track_type: track.track_type.as_str().to_string(),
                result_state: track.result_state.as_str().to_string(),
                marker_count: project
                    .markers
                    .iter()
                    .filter(|marker| marker.track_id == track.id)
                    .count(),
                marker_spans: marker_spans_for_track(project, &track.id, selected_marker_ids),
                error: track.error.clone(),
                active_job_id: latest_job
                    .filter(|job| job.state == ResultState::Running)
                    .map(|job| job.id.clone())
                    .unwrap_or_default(),
                job_state: latest_job
                    .map(|job| job.state.as_str().to_string())
                    .unwrap_or_default(),
                job_progress: latest_job.map_or(0.0, |job| job.progress),
                waveform_samples: Vec::new(),
                cache_ref_count: track.cache_refs.len(),
                artifact_kinds: artifact_kinds_for_track(project, track).join(", "),
                waveform_duration_seconds: waveform_duration_seconds(track),
                editable: track.track_type == TrackType::Editable,
                visible_waveform_samples: visible_waveform_samples(project, track),
                waveform_level_bucket_count: waveform_level_bucket_count(project, track),
                parent_track_id: tree_row.parent_track_id,
                depth: tree_row.depth,
                has_children: tree_row.has_children,
                expanded: tree_row.expanded,
                child_count: tree_row.child_count,
                visible_child_state_summary: tree_row.visible_child_state_summary,
                tree_error: tree_row.tree_error,
                visible_energy_samples: visible_analysis_samples(project, track, "visible_energy"),
                visible_harmonic_color_samples: visible_analysis_samples(
                    project,
                    track,
                    "visible_harmonic_color",
                ),
            })
        })
        .collect()
}

pub fn timeline_rows_json(project: &ProjectDocument) -> Result<String, serde_json::Error> {
    serde_json::to_string(&timeline_rows_for_project(project))
}

pub fn timeline_rows_json_with_state(
    project: &ProjectDocument,
    expanded: &BTreeSet<String>,
    selected_marker_ids: &BTreeSet<String>,
) -> Result<String, serde_json::Error> {
    serde_json::to_string(&timeline_rows_for_project_with_state(
        project,
        expanded,
        selected_marker_ids,
    ))
}

fn source_track() -> Track {
    Track {
        id: "track_source".to_string(),
        track_type: TrackType::Source,
        name: "Rust Demo Source".to_string(),
        input_track_ids: Vec::new(),
        transform_id: String::new(),
        transform_params: JsonObject::new(),
        transform_version: String::new(),
        output_schema: String::new(),
        dependency_hash: String::new(),
        result_state: ResultState::Complete,
        cache_refs: Vec::new(),
        provenance: json_object([("asset_id", json!("asset_demo"))]),
        error: String::new(),
    }
}

fn editable_track() -> Track {
    Track {
        id: "track_edit".to_string(),
        track_type: TrackType::Editable,
        name: "Editable Cues".to_string(),
        input_track_ids: vec!["track_beats".to_string()],
        transform_id: String::new(),
        transform_params: JsonObject::new(),
        transform_version: String::new(),
        output_schema: String::new(),
        dependency_hash: String::new(),
        result_state: ResultState::Complete,
        cache_refs: Vec::new(),
        provenance: json_object([
            ("source_track_id", json!("track_beats")),
            (
                "source_marker_ids",
                json!(["marker_demo_1", "marker_demo_2"]),
            ),
        ]),
        error: String::new(),
    }
}

#[allow(clippy::too_many_arguments)]
fn generated_track(
    id: &str,
    name: &str,
    parent_id: &str,
    transform_id: &str,
    output_schema: &str,
    dependency_hash: &str,
    result_state: ResultState,
    cache_refs: Vec<String>,
    provenance: JsonObject,
) -> Track {
    Track {
        id: id.to_string(),
        track_type: TrackType::Generated,
        name: name.to_string(),
        input_track_ids: vec![parent_id.to_string()],
        transform_id: transform_id.to_string(),
        transform_params: JsonObject::new(),
        transform_version: "1".to_string(),
        output_schema: output_schema.to_string(),
        dependency_hash: dependency_hash.to_string(),
        result_state,
        cache_refs,
        provenance,
        error: String::new(),
    }
}

#[allow(clippy::too_many_arguments)]
fn marker(
    id: &str,
    track_id: &str,
    timestamp: f64,
    duration: Option<f64>,
    label: &str,
    category: &str,
    color: &str,
    source_marker_ids: Vec<String>,
) -> Marker {
    Marker {
        id: id.to_string(),
        track_id: track_id.to_string(),
        timestamp,
        duration,
        label: label.to_string(),
        category: category.to_string(),
        confidence: Some(1.0),
        tags: Vec::new(),
        source_transform: "markers.fixed_interval".to_string(),
        source_marker_ids,
        metadata: json_object([("color", json!(color))]),
    }
}

fn cache_entry(id: &str, artifact_kind: &str, path: &str) -> CacheEntry {
    CacheEntry {
        id: id.to_string(),
        dependency_hash: format!("dep_{artifact_kind}"),
        artifact_kind: artifact_kind.to_string(),
        path: path.to_string(),
        created_at: String::new(),
        transform_version: "1".to_string(),
        size_bytes: 0,
        payload_digest: String::new(),
        validation_status: "valid".to_string(),
    }
}

fn marker_spans_for_track(
    project: &ProjectDocument,
    track_id: &str,
    selected_marker_ids: &BTreeSet<String>,
) -> Vec<MarkerSpan> {
    let mut markers = project
        .markers
        .iter()
        .filter(|marker| marker.track_id == track_id)
        .collect::<Vec<_>>();
    markers.sort_by(|left, right| {
        left.timestamp
            .total_cmp(&right.timestamp)
            .then_with(|| left.id.cmp(&right.id))
    });
    markers
        .into_iter()
        .map(|marker| MarkerSpan {
            id: marker.id.clone(),
            timestamp: marker.timestamp,
            duration: marker.duration.unwrap_or(0.0),
            label: marker.label.clone(),
            category: marker.category.clone(),
            color: marker_display_color(marker),
            selected: selected_marker_ids.contains(&marker.id),
        })
        .collect()
}

fn waveform_duration_seconds(track: &Track) -> f64 {
    track
        .provenance
        .get("visible_waveform")
        .and_then(|value| {
            value
                .get("duration_seconds")
                .or_else(|| value.get("duration"))
        })
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
}

fn visible_waveform_samples(project: &ProjectDocument, track: &Track) -> Vec<Value> {
    if !track_has_valid_complete_artifact(project, track, "waveform") {
        return Vec::new();
    }
    let Some(value) = track.provenance.get("visible_waveform") else {
        return Vec::new();
    };
    let duration = waveform_duration_seconds(track)
        .max(source_audio_duration_seconds(project, track))
        .max(0.0);
    if let Some(samples) = value.get("samples").and_then(Value::as_array) {
        return normalize_waveform_samples(samples, duration);
    }
    normalize_waveform_min_max_arrays(value, duration)
}

fn normalize_waveform_samples(samples: &[Value], duration: f64) -> Vec<Value> {
    let count = samples.len();
    samples
        .iter()
        .enumerate()
        .filter_map(|(index, sample)| {
            let time = sample_time(sample, index, count, duration);
            let peak = sample
                .get("peak")
                .and_then(Value::as_f64)
                .or_else(|| waveform_peak_from_min_max(sample))?;
            let rms = sample
                .get("rms")
                .and_then(Value::as_f64)
                .unwrap_or_else(|| waveform_rms_from_min_max(sample).unwrap_or(peak));
            Some(json!({
                "time": round6(time),
                "peak": clamped_unit(peak),
                "rms": clamped_unit(rms),
            }))
        })
        .collect()
}

fn normalize_waveform_min_max_arrays(value: &Value, duration: f64) -> Vec<Value> {
    let Some(min_values) = value.get("min").and_then(Value::as_array) else {
        return Vec::new();
    };
    let Some(max_values) = value.get("max").and_then(Value::as_array) else {
        return Vec::new();
    };
    let count = value
        .get("buckets")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_else(|| min_values.len().min(max_values.len()));
    let count = count.min(min_values.len()).min(max_values.len());
    (0..count)
        .filter_map(|index| {
            let min_value = min_values[index].as_f64()?;
            let max_value = max_values[index].as_f64()?;
            let peak = min_value.abs().max(max_value.abs());
            let rms = ((min_value.powi(2) + max_value.powi(2)) / 2.0).sqrt();
            Some(json!({
                "time": round6(time_for_index(index, count, duration)),
                "peak": clamped_unit(peak),
                "rms": clamped_unit(rms),
            }))
        })
        .collect()
}

fn waveform_peak_from_min_max(sample: &Value) -> Option<f64> {
    let min_value = sample.get("min").and_then(Value::as_f64)?;
    let max_value = sample.get("max").and_then(Value::as_f64)?;
    Some(min_value.abs().max(max_value.abs()))
}

fn waveform_rms_from_min_max(sample: &Value) -> Option<f64> {
    let min_value = sample.get("min").and_then(Value::as_f64)?;
    let max_value = sample.get("max").and_then(Value::as_f64)?;
    Some(((min_value.powi(2) + max_value.powi(2)) / 2.0).sqrt())
}

fn sample_time(sample: &Value, index: usize, count: usize, duration: f64) -> f64 {
    sample
        .get("time")
        .or_else(|| sample.get("timestamp"))
        .and_then(Value::as_f64)
        .filter(|time| time.is_finite())
        .unwrap_or_else(|| time_for_index(index, count, duration))
}

fn time_for_index(index: usize, count: usize, duration: f64) -> f64 {
    if count == 0 || !duration.is_finite() || duration <= 0.0 {
        return 0.0;
    }
    index as f64 * duration / count as f64
}

fn round6(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn clamped_unit(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    value.clamp(0.0, 1.0)
}

fn source_audio_duration_seconds(project: &ProjectDocument, track: &Track) -> f64 {
    let Some(source_track_id) = source_track_id_for_context(project, &track.id) else {
        return 0.0;
    };
    let Some(source_track) = project
        .tracks
        .iter()
        .find(|candidate| candidate.id == source_track_id)
    else {
        return 0.0;
    };
    let Some(asset_id) = source_track
        .provenance
        .get("asset_id")
        .and_then(Value::as_str)
    else {
        return 0.0;
    };
    project
        .audio_assets
        .iter()
        .find(|asset| asset.id == asset_id)
        .map(|asset| asset.duration)
        .unwrap_or(0.0)
}

fn waveform_level_bucket_count(project: &ProjectDocument, track: &Track) -> usize {
    if !track_has_valid_complete_artifact(project, track, "waveform") {
        return 0;
    }
    track
        .provenance
        .get("visible_waveform")
        .and_then(|value| {
            value
                .get("level_bucket_count")
                .or_else(|| value.get("levelBucketCount"))
                .or_else(|| value.get("buckets"))
        })
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0)
}

fn visible_analysis_samples(project: &ProjectDocument, track: &Track, key: &str) -> Vec<Value> {
    let expected_kind = match key {
        "visible_energy" => "energy",
        "visible_harmonic_color" => "harmonic-color",
        _ => return Vec::new(),
    };
    if !track_has_valid_complete_artifact(project, track, expected_kind) {
        return Vec::new();
    }
    let Some(value) = track.provenance.get(key) else {
        return Vec::new();
    };
    let duration = source_audio_duration_seconds(project, track);
    match key {
        "visible_energy" => normalize_energy_samples(value, duration),
        "visible_harmonic_color" => normalize_harmonic_color_samples(value, duration),
        _ => Vec::new(),
    }
}

fn normalize_energy_samples(value: &Value, duration: f64) -> Vec<Value> {
    if let Some(bins) = value.get("bins").and_then(Value::as_array) {
        let sample_rate = value.get("sample_rate").and_then(Value::as_f64);
        return bins
            .iter()
            .enumerate()
            .filter_map(|(index, bin)| {
                let intensity = bin.as_f64()?;
                let time = sample_rate
                    .filter(|rate| rate.is_finite() && *rate > 0.0)
                    .map(|rate| index as f64 / rate)
                    .unwrap_or_else(|| time_for_index(index, bins.len(), duration));
                Some(json!({
                    "time": round6(time),
                    "intensity": clamped_unit(intensity),
                }))
            })
            .collect();
    }
    let frames = provenance_frames(value);
    frames
        .iter()
        .enumerate()
        .filter_map(|(index, sample)| {
            let time = sample_time(sample, index, frames.len(), duration);
            let intensity = sample
                .get("intensity")
                .or_else(|| sample.get("value"))
                .and_then(Value::as_f64)?;
            let mut frame = json!({
                "time": round6(time),
                "intensity": clamped_unit(intensity),
            });
            if let Some(color) = sample.get("color").and_then(Value::as_str) {
                frame["color"] = json!(color);
            }
            Some(frame)
        })
        .collect()
}

fn normalize_harmonic_color_samples(value: &Value, duration: f64) -> Vec<Value> {
    let frames = provenance_frames(value);
    frames
        .iter()
        .enumerate()
        .map(|(index, sample)| {
            let time = sample_time(sample, index, frames.len(), duration);
            let color = sample
                .get("color")
                .and_then(Value::as_str)
                .unwrap_or("#93c5fd");
            let mut frame = json!({
                "time": round6(time),
                "color": color,
            });
            if let Some(intensity) = sample
                .get("intensity")
                .or_else(|| sample.get("value"))
                .and_then(Value::as_f64)
            {
                frame["intensity"] = json!(clamped_unit(intensity));
            }
            frame
        })
        .collect()
}

fn provenance_frames(value: &Value) -> Vec<Value> {
    value
        .as_array()
        .or_else(|| value.get("frames").and_then(Value::as_array))
        .cloned()
        .unwrap_or_default()
}

fn track_has_valid_complete_artifact(
    project: &ProjectDocument,
    track: &Track,
    expected_kind: &str,
) -> bool {
    if track.result_state != ResultState::Complete {
        return false;
    }
    if track.cache_refs.is_empty() {
        return false;
    }
    track.cache_refs.iter().any(|cache_ref| {
        project.cache_entries.iter().any(|entry| {
            entry.id == *cache_ref
                && entry.artifact_kind == expected_kind
                && entry.validation_status == "valid"
        })
    })
}

fn marker_display_color(marker: &Marker) -> String {
    match marker.metadata.get("color").and_then(Value::as_str) {
        Some("green") => "#a7f3d0",
        Some("amber") => "#fbbf24",
        Some("violet") => "#c4b5fd",
        Some("rose") => "#fda4af",
        Some("blue") => "#93c5fd",
        _ => "#67e8f9",
    }
    .to_string()
}

fn json_object(values: impl IntoIterator<Item = (&'static str, Value)>) -> JsonObject {
    values
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    use serde_json::{json, Value};

    use super::{
        cache_entry, generated_track, json_object, rust_demo_project, timeline_rows_for_project,
        timeline_rows_for_project_with_state, timeline_rows_json, timeline_rows_json_with_state,
    };
    use autolight_core::graph::default_expanded_track_ids;
    use autolight_core::project::{ProjectDocument, ResultState};

    fn fixture_path(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/projects")
            .join(name)
    }

    #[test]
    fn timeline_demo_project_projects_expected_tree_rows() {
        let project = rust_demo_project();
        let rows = timeline_rows_for_project(&project);

        assert_eq!(
            rows.iter()
                .map(|row| row.track_id.as_str())
                .collect::<Vec<_>>(),
            [
                "track_source",
                "track_beats",
                "track_edit",
                "track_waveform",
                "track_drums",
                "track_drum_energy"
            ]
        );
        assert_eq!(
            rows.iter().map(|row| row.depth).collect::<Vec<_>>(),
            [0, 1, 2, 1, 1, 2]
        );
        assert_eq!(rows[0].child_count, 3);
        assert_eq!(rows[4].visible_child_state_summary, "pending: 1");
    }

    #[test]
    fn timeline_rows_include_marker_spans_for_generated_and_editable_tracks() {
        let project = rust_demo_project();
        let rows = timeline_rows_for_project(&project);
        let beats = rows
            .iter()
            .find(|row| row.track_id == "track_beats")
            .unwrap();
        let editable = rows
            .iter()
            .find(|row| row.track_id == "track_edit")
            .unwrap();

        assert_eq!(beats.marker_count, 3);
        assert_eq!(beats.marker_spans[0].timestamp, 0.0);
        assert_eq!(beats.marker_spans[0].color, "#67e8f9");
        assert_eq!(editable.track_type, "editable");
        assert!(editable.editable);
        assert_eq!(editable.marker_spans.len(), 2);
        assert_eq!(editable.marker_spans[0].label, "Cue");
    }

    #[test]
    fn timeline_rows_use_controller_selection_and_expansion_state() {
        let project = rust_demo_project();
        let mut expanded = default_expanded_track_ids(&project);
        let selected = BTreeSet::from(["marker_edit_1".to_string()]);

        expanded.remove("track_drums");
        let rows = timeline_rows_for_project_with_state(&project, &expanded, &selected);

        assert!(!rows.iter().any(|row| row.track_id == "track_drum_energy"));
        let drums = rows
            .iter()
            .find(|row| row.track_id == "track_drums")
            .unwrap();
        let editable = rows
            .iter()
            .find(|row| row.track_id == "track_edit")
            .unwrap();
        assert!(!drums.expanded);
        assert!(editable.marker_spans[0].selected);
        assert!(!editable.marker_spans[1].selected);

        let payload = timeline_rows_json_with_state(&project, &expanded, &selected).unwrap();
        let json_rows: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(json_rows[2]["markerSpans"][0]["selected"], true);
    }

    #[test]
    fn timeline_rows_json_contains_qml_role_names() {
        let project = rust_demo_project();
        let payload = timeline_rows_json(&project).unwrap();
        let rows: Value = serde_json::from_str(&payload).unwrap();
        let first = &rows[0];

        assert_eq!(first["trackId"], "track_source");
        assert_eq!(first["markerSpans"], Value::Array(Vec::new()));
        assert!(first.get("visibleEnergySamples").is_some());
        assert!(first.get("visibleHarmonicColorSamples").is_some());
        assert!(first.get("waveformDurationSeconds").is_some());
        assert!(first.get("activeJobId").is_some());
        assert!(first.get("jobState").is_some());
        assert!(first.get("jobProgress").is_some());
        assert!(first.get("cacheRefCount").is_some());
        assert!(first.get("artifactKinds").is_some());
    }

    #[test]
    fn timeline_rows_expose_waveform_bucket_count() {
        let project = rust_demo_project();
        let rows = timeline_rows_for_project(&project);
        let waveform = rows
            .iter()
            .find(|row| row.track_id == "track_waveform")
            .unwrap();

        assert_eq!(waveform.waveform_level_bucket_count, 64);
        assert_eq!(waveform.visible_waveform_samples.len(), 2);
        assert_eq!(waveform.visible_waveform_samples[0]["time"], json!(0.0));
        assert_eq!(waveform.visible_waveform_samples[0]["peak"], json!(0.2));
        assert!(waveform.visible_waveform_samples[0].get("rms").is_some());
    }

    #[test]
    fn timeline_rows_normalize_legacy_waveform_and_energy_payloads() {
        let project = ProjectDocument::load_path(fixture_path("tree_analysis.autolight")).unwrap();
        let rows = timeline_rows_for_project(&project);
        let waveform = rows
            .iter()
            .find(|row| row.track_id == "track_waveform")
            .unwrap();
        let energy = rows
            .iter()
            .find(|row| row.track_id == "track_energy")
            .unwrap();

        assert_eq!(waveform.waveform_level_bucket_count, 3);
        assert_eq!(waveform.visible_waveform_samples.len(), 3);
        assert_eq!(waveform.visible_waveform_samples[1]["time"], json!(10.0));
        assert_eq!(waveform.visible_waveform_samples[1]["peak"], json!(0.5));
        assert!(waveform.visible_waveform_samples[1].get("rms").is_some());
        assert_eq!(energy.visible_energy_samples.len(), 3);
        assert_eq!(energy.visible_energy_samples[1]["time"], json!(0.1));
        assert_eq!(energy.visible_energy_samples[1]["intensity"], json!(0.8));
    }

    #[test]
    fn timeline_rows_emit_drawable_analysis_samples() {
        let mut project = rust_demo_project();
        project
            .tracks
            .iter_mut()
            .find(|track| track.id == "track_drum_energy")
            .unwrap()
            .result_state = ResultState::Complete;
        project.cache_entries.push(cache_entry(
            "cache_harmonic",
            "harmonic-color",
            "cache/harmonic/rust-demo.json",
        ));
        project.tracks.push(generated_track(
            "track_harmonic",
            "Harmonic Color",
            "track_source",
            "music.harmonic_color",
            "artifact.harmonic-color.v1",
            "dep_harmonic",
            ResultState::Complete,
            vec!["cache_harmonic".to_string()],
            json_object([(
                "visible_harmonic_color",
                json!([
                    {"timestamp": 0.0, "color": "#f00"},
                    {"time": 0.5, "color": "#0f0", "intensity": 0.75}
                ]),
            )]),
        ));

        let rows = timeline_rows_for_project(&project);
        let energy = rows
            .iter()
            .find(|row| row.track_id == "track_drum_energy")
            .unwrap();
        let harmonic = rows
            .iter()
            .find(|row| row.track_id == "track_harmonic")
            .unwrap();

        assert_eq!(energy.visible_energy_samples[0]["time"], json!(0.0));
        assert_eq!(energy.visible_energy_samples[0]["intensity"], json!(0.2));
        assert_eq!(
            harmonic.visible_harmonic_color_samples[0]["time"],
            json!(0.0)
        );
        assert_eq!(
            harmonic.visible_harmonic_color_samples[0]["color"],
            json!("#f00")
        );
        assert_eq!(
            harmonic.visible_harmonic_color_samples[1]["intensity"],
            json!(0.75)
        );
    }

    #[test]
    fn timeline_rows_hide_analysis_samples_without_valid_complete_artifacts() {
        let mut project = rust_demo_project();

        let rows = timeline_rows_for_project(&project);
        let pending_energy = rows
            .iter()
            .find(|row| row.track_id == "track_drum_energy")
            .unwrap();
        assert!(pending_energy.visible_energy_samples.is_empty());

        project
            .tracks
            .iter_mut()
            .find(|track| track.id == "track_drum_energy")
            .unwrap()
            .result_state = ResultState::Complete;
        project
            .cache_entries
            .iter_mut()
            .find(|entry| entry.id == "cache_energy")
            .unwrap()
            .validation_status = "invalid".to_string();

        let rows = timeline_rows_for_project(&project);
        let invalid_energy = rows
            .iter()
            .find(|row| row.track_id == "track_drum_energy")
            .unwrap();
        assert!(invalid_energy.visible_energy_samples.is_empty());

        project
            .cache_entries
            .iter_mut()
            .find(|entry| entry.id == "cache_waveform")
            .unwrap()
            .validation_status = "invalid".to_string();
        let rows = timeline_rows_for_project(&project);
        let invalid_waveform = rows
            .iter()
            .find(|row| row.track_id == "track_waveform")
            .unwrap();
        assert!(invalid_waveform.visible_waveform_samples.is_empty());
        assert_eq!(invalid_waveform.waveform_level_bucket_count, 0);
    }
}
