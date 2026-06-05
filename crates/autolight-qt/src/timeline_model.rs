use std::collections::{BTreeMap, BTreeSet};

use autolight_core::graph::{
    default_expanded_track_ids, project_tree, source_track_id_for_context,
};
use autolight_core::project::{
    AudioAsset, CacheEntry, CacheValidationStatus, ImportStatus, JobRun, JsonObject, Marker,
    ProjectDocument, ResultState, Track, TrackType,
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
    pub cache_ref_count: usize,
    pub artifact_kinds: String,
    pub waveform_duration_seconds: f64,
    pub waveform_ref: Option<TimelineWaveformRef>,
    pub editable: bool,
    pub parent_track_id: String,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
    pub child_count: usize,
    pub visible_child_state_summary: String,
    pub tree_error: String,
    pub analysis_refs: Vec<TimelineAnalysisRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineWaveformRef {
    pub track_id: String,
    pub cache_ref: String,
    pub artifact_kind: String,
    pub duration_seconds: f64,
    pub sample_rate: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineAnalysisRef {
    pub track_id: String,
    pub cache_ref: String,
    pub artifact_kind: String,
    pub duration_seconds: f64,
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

struct TimelineProjectionContext<'a> {
    project: &'a ProjectDocument,
    tracks_by_id: BTreeMap<&'a str, &'a Track>,
    markers_by_track: BTreeMap<&'a str, Vec<&'a Marker>>,
    latest_job_by_track: BTreeMap<&'a str, &'a JobRun>,
    cache_entries_by_id: BTreeMap<&'a str, &'a CacheEntry>,
    audio_assets_by_id: BTreeMap<&'a str, &'a AudioAsset>,
}

impl<'a> TimelineProjectionContext<'a> {
    fn new(project: &'a ProjectDocument) -> Self {
        let tracks_by_id = project
            .tracks
            .iter()
            .map(|track| (track.id.as_str(), track))
            .collect();
        let mut markers_by_track: BTreeMap<&str, Vec<&Marker>> = BTreeMap::new();
        for marker in &project.markers {
            markers_by_track
                .entry(marker.track_id.as_str())
                .or_default()
                .push(marker);
        }
        for markers in markers_by_track.values_mut() {
            markers.sort_by(|left, right| {
                left.timestamp
                    .total_cmp(&right.timestamp)
                    .then_with(|| left.id.cmp(&right.id))
            });
        }
        let mut latest_job_by_track: BTreeMap<&str, &JobRun> = BTreeMap::new();
        for run in &project.job_runs {
            latest_job_by_track
                .entry(run.track_id.as_str())
                .and_modify(|current| {
                    if job_run_is_newer(run, current) {
                        *current = run;
                    }
                })
                .or_insert(run);
        }
        let cache_entries_by_id = project
            .cache_entries
            .iter()
            .map(|entry| (entry.id.as_str(), entry))
            .collect();
        let audio_assets_by_id = project
            .audio_assets
            .iter()
            .map(|asset| (asset.id.as_str(), asset))
            .collect();
        Self {
            project,
            tracks_by_id,
            markers_by_track,
            latest_job_by_track,
            cache_entries_by_id,
            audio_assets_by_id,
        }
    }

    fn track(&self, track_id: &str) -> Option<&'a Track> {
        self.tracks_by_id.get(track_id).copied()
    }

    fn markers_for_track(&self, track_id: &str) -> &[&'a Marker] {
        self.markers_by_track
            .get(track_id)
            .map_or(&[], Vec::as_slice)
    }

    fn latest_job_for_track(&self, track_id: &str) -> Option<&'a JobRun> {
        self.latest_job_by_track.get(track_id).copied()
    }

    fn artifact_kinds_for_track(&self, track: &Track) -> Vec<String> {
        track
            .cache_refs
            .iter()
            .filter_map(|cache_ref| self.cache_entries_by_id.get(cache_ref.as_str()))
            .map(|entry| entry.artifact_kind.clone())
            .collect()
    }

    fn valid_complete_artifact_for_track(
        &self,
        track: &Track,
        expected_kind: &str,
    ) -> Option<&'a CacheEntry> {
        if track.result_state != ResultState::Complete || track.cache_refs.is_empty() {
            return None;
        }
        track.cache_refs.iter().find_map(|cache_ref| {
            self.cache_entries_by_id
                .get(cache_ref.as_str())
                .copied()
                .filter(|entry| {
                    entry.artifact_kind == expected_kind
                        && entry.validation_status == CacheValidationStatus::Valid
                })
        })
    }

    fn source_audio_duration_seconds(&self, track: &Track) -> f64 {
        let Some(source_track_id) = source_track_id_for_context(self.project, &track.id) else {
            return 0.0;
        };
        let Some(source_track) = self.track(&source_track_id) else {
            return 0.0;
        };
        let Some(asset_id) = source_track
            .provenance
            .get("asset_id")
            .and_then(Value::as_str)
        else {
            return 0.0;
        };
        self.audio_assets_by_id
            .get(asset_id)
            .map_or(0.0, |asset| asset.duration)
    }

    fn source_audio_sample_rate(&self, track: &Track) -> u32 {
        let Some(source_track_id) = source_track_id_for_context(self.project, &track.id) else {
            return 0;
        };
        let Some(source_track) = self.track(&source_track_id) else {
            return 0;
        };
        let Some(asset_id) = source_track
            .provenance
            .get("asset_id")
            .and_then(Value::as_str)
        else {
            return 0;
        };
        self.audio_assets_by_id
            .get(asset_id)
            .map_or(0, |asset| asset.sample_rate)
    }
}

fn job_run_is_newer(candidate: &JobRun, current: &JobRun) -> bool {
    let candidate_rank = job_run_state_rank(candidate.state);
    let current_rank = job_run_state_rank(current.state);
    candidate_rank > current_rank
        || (candidate_rank == current_rank && candidate.id.as_str() > current.id.as_str())
}

fn job_run_state_rank(state: ResultState) -> u8 {
    match state {
        ResultState::Running => 5,
        ResultState::Pending => 4,
        ResultState::Failed | ResultState::Cancelled => 3,
        ResultState::Complete => 2,
        ResultState::Blocked => 2,
        ResultState::Stale => 1,
    }
}

pub fn rust_demo_project() -> ProjectDocument {
    let mut project = ProjectDocument::new("project_rust_demo", RUST_DEMO_PROJECT_NAME);
    project.audio_assets.push(AudioAsset {
        id: "asset_demo".to_string(),
        path: "rust-demo.wav".to_string(),
        duration: 2.0,
        sample_rate: 44_100,
        channels: 2,
        fingerprint: "rust-demo-fingerprint".to_string(),
        import_status: ImportStatus::Offline,
        relink_hint: "rust-demo.wav".to_string(),
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
            Vec::default(),
            JsonObject::default(),
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
            json_object([
                ("waveform_payload", demo_waveform_payload()),
                (
                    "visible_waveform",
                    json!({
                        "duration_seconds": 2.0,
                        "level_bucket_count": 64,
                        "samples": [
                            {"min": -0.1, "max": 0.2},
                            {"min": -0.3, "max": 0.4}
                        ]
                    }),
                ),
            ]),
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
            ResultState::Complete,
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
            Vec::default(),
        ),
        marker(
            "marker_demo_2",
            "track_beats",
            0.5,
            None,
            "Beat",
            "timing",
            "green",
            Vec::default(),
        ),
        marker(
            "marker_demo_3",
            "track_beats",
            1.0,
            None,
            "Beat",
            "timing",
            "blue",
            Vec::default(),
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
    let context = TimelineProjectionContext::new(project);

    tree_rows
        .into_iter()
        .filter_map(|tree_row| {
            let track = context.track(tree_row.track_id.as_str())?;
            let latest_job = context.latest_job_for_track(&track.id);
            Some(TimelineRow {
                track_id: track.id.clone(),
                name: track.name.clone(),
                track_type: track.track_type.as_str().to_string(),
                result_state: track.result_state.as_str().to_string(),
                marker_count: context.markers_for_track(&track.id).len(),
                marker_spans: marker_spans_for_track(&context, &track.id, selected_marker_ids),
                error: track.error.clone(),
                active_job_id: latest_job
                    .filter(|job| matches!(job.state, ResultState::Pending | ResultState::Running))
                    .map(|job| job.id.clone())
                    .unwrap_or_default(),
                job_state: latest_job
                    .map(|job| job.state.as_str().to_string())
                    .unwrap_or_default(),
                job_progress: latest_job.map_or(0.0, |job| job.progress),
                cache_ref_count: track.cache_refs.len(),
                artifact_kinds: context.artifact_kinds_for_track(track).join(", "),
                waveform_duration_seconds: waveform_duration_seconds(track),
                waveform_ref: waveform_ref(&context, track),
                editable: track.track_type == TrackType::Editable,
                parent_track_id: tree_row.parent_track_id,
                depth: tree_row.depth,
                has_children: tree_row.has_children,
                expanded: tree_row.expanded,
                child_count: tree_row.child_count,
                visible_child_state_summary: tree_row.visible_child_state_summary,
                tree_error: tree_row.tree_error,
                analysis_refs: analysis_refs(&context, track),
            })
        })
        .collect()
}

pub fn timeline_track_ids_for_project_with_state(
    project: &ProjectDocument,
    expanded: &BTreeSet<String>,
) -> Vec<String> {
    let context = TimelineProjectionContext::new(project);
    project_tree(project, expanded)
        .into_iter()
        .filter_map(|tree_row| {
            context
                .track(tree_row.track_id.as_str())
                .map(|track| track.id.clone())
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
        input_track_ids: Vec::default(),
        transform_id: String::default(),
        transform_params: JsonObject::default(),
        transform_version: String::default(),
        output_schema: String::default(),
        dependency_hash: String::default(),
        result_state: ResultState::Complete,
        cache_refs: Vec::default(),
        provenance: json_object([("asset_id", json!("asset_demo"))]),
        error: String::default(),
    }
}

fn editable_track() -> Track {
    Track {
        id: "track_edit".to_string(),
        track_type: TrackType::Editable,
        name: "Editable Cues".to_string(),
        input_track_ids: vec!["track_beats".to_string()],
        transform_id: String::default(),
        transform_params: JsonObject::default(),
        transform_version: String::default(),
        output_schema: String::default(),
        dependency_hash: String::default(),
        result_state: ResultState::Complete,
        cache_refs: Vec::default(),
        provenance: json_object([
            ("source_track_id", json!("track_beats")),
            (
                "source_marker_ids",
                json!(["marker_demo_1", "marker_demo_2"]),
            ),
        ]),
        error: String::default(),
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
        transform_params: JsonObject::default(),
        transform_version: "1".to_string(),
        output_schema: output_schema.to_string(),
        dependency_hash: dependency_hash.to_string(),
        result_state,
        cache_refs,
        provenance,
        error: String::default(),
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
        tags: Vec::default(),
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
        created_at: String::default(),
        transform_version: "1".to_string(),
        size_bytes: 0,
        payload_digest: String::default(),
        validation_status: CacheValidationStatus::Valid,
    }
}

fn demo_waveform_payload() -> Value {
    let samples = (0..64)
        .map(|index| {
            let peak = 0.18 + (index % 8) as f64 * 0.08;
            json!({
                "peak": peak.min(0.86),
                "rms": (peak * 0.55).min(0.5)
            })
        })
        .collect::<Vec<_>>();
    json!({
        "version": 2,
        "sample_rate": 32,
        "duration": 2.0,
        "samples": samples,
        "levels": [
            {
                "bucket_count": 64,
                "samples": samples
            }
        ]
    })
}

fn marker_spans_for_track(
    context: &TimelineProjectionContext<'_>,
    track_id: &str,
    selected_marker_ids: &BTreeSet<String>,
) -> Vec<MarkerSpan> {
    context
        .markers_for_track(track_id)
        .iter()
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
    if let Some(duration) = track
        .provenance
        .get("waveform_payload")
        .and_then(|value| value.get("duration"))
        .and_then(Value::as_f64)
    {
        return duration;
    }
    if let Some(duration) = track
        .provenance
        .get("waveform_duration_seconds")
        .and_then(Value::as_f64)
    {
        return duration;
    }
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

fn waveform_ref(
    context: &TimelineProjectionContext<'_>,
    track: &Track,
) -> Option<TimelineWaveformRef> {
    let entry = context.valid_complete_artifact_for_track(track, "waveform")?;
    let duration_seconds = waveform_duration_seconds(track)
        .max(context.source_audio_duration_seconds(track))
        .max(0.0);
    Some(TimelineWaveformRef {
        track_id: track.id.clone(),
        cache_ref: entry.id.clone(),
        artifact_kind: entry.artifact_kind.clone(),
        duration_seconds,
        sample_rate: waveform_sample_rate(context, track),
    })
}

fn waveform_sample_rate(context: &TimelineProjectionContext<'_>, track: &Track) -> u32 {
    track
        .provenance
        .get("waveform_payload")
        .and_then(|value| value.get("sample_rate").or_else(|| value.get("sampleRate")))
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or_else(|| context.source_audio_sample_rate(track))
}

fn analysis_refs(
    context: &TimelineProjectionContext<'_>,
    track: &Track,
) -> Vec<TimelineAnalysisRef> {
    ["visible_energy", "visible_harmonic_color"]
        .into_iter()
        .filter_map(|key| analysis_ref_for_key(context, track, key))
        .collect()
}

fn analysis_ref_for_key(
    context: &TimelineProjectionContext<'_>,
    track: &Track,
    key: &str,
) -> Option<TimelineAnalysisRef> {
    let expected_kind = match key {
        "visible_energy" => "energy",
        "visible_harmonic_color" => "harmonic-color",
        _ => return None,
    };
    let entry = context.valid_complete_artifact_for_track(track, expected_kind)?;
    Some(TimelineAnalysisRef {
        track_id: track.id.clone(),
        cache_ref: entry.id.clone(),
        artifact_kind: entry.artifact_kind.clone(),
        duration_seconds: context.source_audio_duration_seconds(track),
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
    use autolight_core::project::{
        CacheValidationStatus, ImportStatus, JobRun, JsonObject, ProjectDocument, ResultState,
    };

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
        assert!(rows[4].visible_child_state_summary.is_empty());
    }

    #[test]
    fn timeline_demo_project_has_no_fake_active_job_or_online_audio() {
        let project = rust_demo_project();

        assert!(project.job_runs.is_empty());
        assert_eq!(project.audio_assets[0].import_status, ImportStatus::Offline);
        assert_eq!(project.audio_assets[0].relink_hint, "rust-demo.wav");

        let rows = timeline_rows_for_project(&project);
        let energy = rows
            .iter()
            .find(|row| row.track_id == "track_drum_energy")
            .unwrap();
        assert_eq!(energy.result_state, "complete");
        assert!(energy.active_job_id.is_empty());
        assert!(energy.job_state.is_empty());
        assert_eq!(energy.analysis_refs[0].artifact_kind, "energy");
    }

    #[test]
    fn timeline_rows_mark_pending_jobs_active_for_polling() {
        let mut project = rust_demo_project();
        project
            .tracks
            .iter_mut()
            .find(|track| track.id == "track_waveform")
            .unwrap()
            .result_state = ResultState::Pending;
        project.job_runs.push(JobRun {
            id: "job_pending".to_string(),
            track_id: "track_waveform".to_string(),
            transform_id: "waveform.summary".to_string(),
            transform_version: "1".to_string(),
            parameters_hash: "hash".to_string(),
            parameters: JsonObject::default(),
            state: ResultState::Pending,
            progress: 0.0,
            started_at: String::default(),
            completed_at: String::default(),
            error: String::default(),
            produced_cache_refs: Vec::default(),
        });

        let rows = timeline_rows_for_project(&project);
        let waveform = rows
            .iter()
            .find(|row| row.track_id == "track_waveform")
            .unwrap();

        assert_eq!(waveform.active_job_id, "job_pending");
        assert_eq!(waveform.job_state, "pending");
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
    fn timeline_rows_json_omits_unused_legacy_waveform_samples_field() {
        let project = rust_demo_project();
        let rows: Vec<Value> =
            serde_json::from_str(&timeline_rows_json(&project).unwrap()).unwrap();

        assert!(rows.iter().all(|row| row.get("waveformSamples").is_none()));
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
        assert!(json_rows[2]["markerSpans"][0]["selected"]
            .as_bool()
            .unwrap_or(false));
    }

    #[test]
    fn timeline_rows_json_contains_qml_role_names() {
        let project = rust_demo_project();
        let payload = timeline_rows_json(&project).unwrap();
        let rows: Value = serde_json::from_str(&payload).unwrap();
        let first = &rows[0];

        assert_eq!(first["trackId"], "track_source");
        assert_eq!(first["markerSpans"], Value::Array(Vec::default()));
        assert!(first.get("analysisRefs").is_some());
        assert!(first.get("visibleEnergySamples").is_none());
        assert!(first.get("visibleHarmonicColorSamples").is_none());
        assert!(first.get("waveformDurationSeconds").is_some());
        assert!(first.get("activeJobId").is_some());
        assert!(first.get("jobState").is_some());
        assert!(first.get("jobProgress").is_some());
        assert!(first.get("cacheRefCount").is_some());
        assert!(first.get("artifactKinds").is_some());
    }

    #[test]
    fn timeline_rows_emit_waveform_ref_without_embedded_levels() {
        let project = rust_demo_project();
        let payload: Value = serde_json::from_str(&timeline_rows_json(&project).unwrap()).unwrap();
        let waveform = payload
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["trackId"] == json!("track_waveform"))
            .unwrap();

        assert_eq!(waveform["waveformRef"]["trackId"], json!("track_waveform"));
        assert_eq!(waveform["waveformRef"]["cacheRef"], json!("cache_waveform"));
        assert_eq!(waveform["waveformRef"]["artifactKind"], json!("waveform"));
        assert_eq!(waveform["waveformRef"]["durationSeconds"], json!(2.0));
        assert_eq!(waveform["waveformRef"]["sampleRate"], json!(32));
        assert!(waveform.get("waveformLevels").is_none());
        assert!(waveform.get("visibleWaveformSamples").is_none());
    }

    #[test]
    fn timeline_rows_emit_waveform_ref_duration_from_full_payload_over_stale_visible_slice() {
        let mut project = rust_demo_project();
        let waveform = project
            .tracks
            .iter_mut()
            .find(|track| track.id == "track_waveform")
            .unwrap();
        waveform.provenance.insert(
            "waveform_payload".to_string(),
            json!({
                "version": 2,
                "sample_rate": 4,
                "duration": 4.0,
                "samples": [
                    {"peak": 0.1, "rms": 0.05},
                    {"peak": 0.3, "rms": 0.15},
                    {"peak": 0.5, "rms": 0.25},
                    {"peak": 0.7, "rms": 0.35}
                ],
                "levels": [
                    {
                        "bucket_count": 4,
                        "samples": [
                            {"peak": 0.1, "rms": 0.05},
                            {"peak": 0.3, "rms": 0.15},
                            {"peak": 0.5, "rms": 0.25},
                            {"peak": 0.7, "rms": 0.35}
                        ]
                    }
                ]
            }),
        );
        waveform.provenance.insert(
            "visible_waveform".to_string(),
            json!({
                "duration_seconds": 1.0,
                "level_bucket_count": 1,
                "samples": [{"time": 0.0, "peak": 0.1, "rms": 0.05}]
            }),
        );

        let rows = timeline_rows_for_project(&project);
        let waveform = rows
            .iter()
            .find(|row| row.track_id == "track_waveform")
            .unwrap();

        assert_eq!(waveform.waveform_duration_seconds, 4.0);
        let waveform_ref = waveform.waveform_ref.as_ref().unwrap();
        assert_eq!(waveform_ref.duration_seconds, 4.0);
        assert_eq!(waveform_ref.cache_ref, "cache_waveform");
        assert_eq!(waveform_ref.sample_rate, 4);
    }

    #[test]
    fn timeline_rows_json_omits_waveform_samples_for_multi_level_payload() {
        let mut project = rust_demo_project();
        let waveform = project
            .tracks
            .iter_mut()
            .find(|track| track.id == "track_waveform")
            .unwrap();
        waveform.provenance.insert(
            "waveform_payload".to_string(),
            json!({
                "version": 2,
                "sample_rate": 64,
                "duration": 8.0,
                "samples": [
                    {"peak": 0.1, "rms": 0.05},
                    {"peak": 0.2, "rms": 0.10}
                ],
                "levels": [
                    {
                        "bucket_count": 2,
                        "samples": [
                            {"peak": 0.1, "rms": 0.05},
                            {"peak": 0.2, "rms": 0.10}
                        ]
                    },
                    {
                        "bucket_count": 8,
                        "samples": [
                            {"peak": 0.1, "rms": 0.05},
                            {"peak": 0.2, "rms": 0.10},
                            {"peak": 0.3, "rms": 0.15},
                            {"peak": 0.4, "rms": 0.20},
                            {"peak": 0.5, "rms": 0.25},
                            {"peak": 0.6, "rms": 0.30},
                            {"peak": 0.7, "rms": 0.35},
                            {"peak": 0.8, "rms": 0.40}
                        ]
                    }
                ]
            }),
        );

        let payload: Value = serde_json::from_str(&timeline_rows_json(&project).unwrap()).unwrap();
        let waveform = payload
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["trackId"] == json!("track_waveform"))
            .unwrap();

        assert!(waveform["waveformRef"].is_object());
        assert!(waveform.get("waveformLevels").is_none());
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

        assert!(waveform.waveform_ref.is_some());
        assert!(energy
            .analysis_refs
            .iter()
            .any(|reference| reference.artifact_kind == "energy"));
    }

    #[test]
    fn timeline_rows_emit_analysis_refs_without_visible_canvas_samples() {
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

        assert_eq!(energy.analysis_refs[0].artifact_kind, "energy");
        assert_eq!(harmonic.analysis_refs[0].artifact_kind, "harmonic-color");
    }

    #[test]
    fn timeline_rows_omit_refs_for_invalid_or_incomplete_cache() {
        let mut project = rust_demo_project();

        let rows = timeline_rows_for_project(&project);
        let pending_energy = rows
            .iter()
            .find(|row| row.track_id == "track_drum_energy")
            .unwrap();
        assert_eq!(pending_energy.result_state, "complete");
        assert!(!pending_energy.analysis_refs.is_empty());

        project
            .tracks
            .iter_mut()
            .find(|track| track.id == "track_drum_energy")
            .unwrap()
            .result_state = ResultState::Pending;
        let rows = timeline_rows_for_project(&project);
        let pending_energy = rows
            .iter()
            .find(|row| row.track_id == "track_drum_energy")
            .unwrap();
        assert!(pending_energy.analysis_refs.is_empty());

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
            .validation_status = CacheValidationStatus::Invalid;

        let rows = timeline_rows_for_project(&project);
        let invalid_energy = rows
            .iter()
            .find(|row| row.track_id == "track_drum_energy")
            .unwrap();
        assert!(invalid_energy.analysis_refs.is_empty());

        project
            .cache_entries
            .iter_mut()
            .find(|entry| entry.id == "cache_waveform")
            .unwrap()
            .validation_status = CacheValidationStatus::Invalid;
        let rows = timeline_rows_for_project(&project);
        let invalid_waveform = rows
            .iter()
            .find(|row| row.track_id == "track_waveform")
            .unwrap();
        assert!(invalid_waveform.waveform_ref.is_none());
    }
}
