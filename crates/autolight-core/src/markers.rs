use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};
use thiserror::Error;

use crate::graph::{find_track, mark_dependents_stale, source_track_id_for_context};
use crate::project::{JsonObject, Marker, ProjectDocument, ResultState, Track, TrackType};

const DEFAULT_MARKER_COLOR: &str = "cyan";
const MARKER_COLOR_KEYS: &[&str] = &["cyan", "green", "amber", "violet", "rose", "blue"];

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Error)]
pub enum MarkerError {
    #[error("track not found: {0}")]
    TrackNotFound(String),
    #[error("markers can only be edited on an editable track")]
    NotEditableTrack,
    #[error("manual cue tracks require a source audio context")]
    SourceAudioContextRequired,
    #[error("marker not found on track {track_id}: {marker_id}")]
    MarkerNotFound { track_id: String, marker_id: String },
    #[error("marker timestamp must be finite")]
    TimestampNotFinite,
    #[error("marker delta must be finite")]
    DeltaNotFinite,
    #[error("marker move would create a non-finite timestamp")]
    MoveNotFinite,
    #[error("marker move would create a negative timestamp")]
    NegativeTimestamp,
    #[error("marker duration must be finite")]
    DurationNotFinite,
    #[error("marker duration must be greater than or equal to zero")]
    NegativeDuration,
    #[error("marker color must be one of: {0}")]
    InvalidColor(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditableMarkerInput {
    pub timestamp: f64,
    pub duration: Option<f64>,
    pub label: String,
    pub category: String,
    pub color: String,
}

impl EditableMarkerInput {
    pub fn cue(timestamp: f64, label: impl Into<String>) -> Self {
        Self {
            timestamp,
            duration: None,
            label: label.into(),
            category: "cue".to_string(),
            color: DEFAULT_MARKER_COLOR.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarkerUpdate {
    pub timestamp: f64,
    pub duration: Option<f64>,
    pub label: String,
    pub category: String,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkMarkerUpdate {
    pub label: String,
    pub category: String,
    pub color: String,
}

pub fn create_manual_editable_track(
    project: &mut ProjectDocument,
    context_track_id: &str,
    name: &str,
) -> Result<Track, MarkerError> {
    let source_track_id = source_track_id_for_context(project, context_track_id)
        .ok_or(MarkerError::SourceAudioContextRequired)?;
    let track = Track {
        id: new_id("track"),
        track_type: TrackType::Editable,
        name: if name.is_empty() {
            "Manual Cues".to_string()
        } else {
            name.to_string()
        },
        input_track_ids: vec![source_track_id.clone()],
        transform_id: String::new(),
        transform_params: JsonObject::new(),
        transform_version: String::new(),
        output_schema: String::new(),
        dependency_hash: String::new(),
        result_state: ResultState::Complete,
        cache_refs: Vec::new(),
        provenance: json_object([
            ("source_track_id", json!(source_track_id)),
            ("manual_track", json!(true)),
            ("created_by", json!("user")),
        ]),
        error: String::new(),
    };
    project.tracks.push(track.clone());
    Ok(track)
}

pub fn add_editable_marker(
    project: &mut ProjectDocument,
    track_id: &str,
    input: EditableMarkerInput,
) -> Result<Marker, MarkerError> {
    editable_track_or_raise(project, track_id)?;
    let timestamp = finite_marker_timestamp(input.timestamp)?;
    let duration = optional_marker_duration(input.duration)?;
    let color = normalize_marker_color(&input.color)?;
    let marker = Marker {
        id: new_id("marker"),
        track_id: track_id.to_string(),
        timestamp,
        duration,
        label: input.label,
        category: normalize_marker_category(&input.category),
        confidence: None,
        tags: Vec::new(),
        source_transform: String::new(),
        source_marker_ids: Vec::new(),
        metadata: json_object([
            ("created_by", json!("user")),
            ("color", json!(color.as_str())),
        ]),
    };
    project.markers.push(marker.clone());
    mark_dependents_stale(project, track_id, "");
    Ok(marker)
}

pub fn update_editable_marker(
    project: &mut ProjectDocument,
    track_id: &str,
    marker_id: &str,
    update: MarkerUpdate,
) -> Result<Marker, MarkerError> {
    editable_track_or_raise(project, track_id)?;
    let timestamp = finite_marker_timestamp(update.timestamp)?;
    let duration = optional_marker_duration(update.duration)?;
    let category = normalize_marker_category(&update.category);
    let color = normalize_marker_color(&update.color)?;
    let marker_index = marker_index_or_raise(project, track_id, marker_id)?;

    let changed = {
        let marker = &mut project.markers[marker_index];
        apply_marker_fields(
            marker,
            timestamp,
            duration,
            &update.label,
            &category,
            &color,
        )
    };
    let marker = project.markers[marker_index].clone();
    if changed {
        mark_dependents_stale(project, track_id, "");
    }
    Ok(marker)
}

pub fn bulk_update_editable_markers(
    project: &mut ProjectDocument,
    track_id: &str,
    marker_ids: &[String],
    update: BulkMarkerUpdate,
) -> Result<usize, MarkerError> {
    editable_track_or_raise(project, track_id)?;
    let selected_ids: BTreeSet<&str> = marker_ids.iter().map(String::as_str).collect();
    let category = normalize_marker_category(&update.category);
    let color = normalize_marker_color(&update.color)?;
    let mut changed_count = 0;

    for marker in &mut project.markers {
        if marker.track_id != track_id {
            continue;
        }
        if !selected_ids.is_empty() && !selected_ids.contains(marker.id.as_str()) {
            continue;
        }
        if apply_bulk_marker_fields(marker, &update.label, &category, &color) {
            changed_count += 1;
        }
    }

    if changed_count > 0 {
        mark_dependents_stale(project, track_id, "");
    }
    Ok(changed_count)
}

pub fn move_editable_markers(
    project: &mut ProjectDocument,
    track_id: &str,
    marker_ids: &[String],
    delta_seconds: f64,
) -> Result<Vec<Marker>, MarkerError> {
    editable_track_or_raise(project, track_id)?;
    let delta = finite_marker_delta(delta_seconds)?;
    let marker_indices = marker_indices_or_raise(project, track_id, marker_ids)?;
    let mut next_timestamps = Vec::with_capacity(marker_indices.len());

    for marker_index in &marker_indices {
        let timestamp = finite_movable_marker_timestamp(project.markers[*marker_index].timestamp)?;
        let next_timestamp = timestamp + delta;
        if !next_timestamp.is_finite() {
            return Err(MarkerError::MoveNotFinite);
        }
        if next_timestamp < 0.0 {
            return Err(MarkerError::NegativeTimestamp);
        }
        next_timestamps.push(next_timestamp);
    }

    let changed =
        marker_indices
            .iter()
            .zip(&next_timestamps)
            .any(|(marker_index, next_timestamp)| {
                project.markers[*marker_index].timestamp != *next_timestamp
            });
    for (marker_index, next_timestamp) in marker_indices.iter().zip(next_timestamps) {
        project.markers[*marker_index].timestamp = next_timestamp;
    }
    let moved = marker_indices
        .into_iter()
        .map(|marker_index| project.markers[marker_index].clone())
        .collect();
    if changed {
        mark_dependents_stale(project, track_id, "");
    }
    Ok(moved)
}

pub fn resize_editable_marker(
    project: &mut ProjectDocument,
    track_id: &str,
    marker_id: &str,
    duration: f64,
) -> Result<Marker, MarkerError> {
    editable_track_or_raise(project, track_id)?;
    let duration = finite_marker_duration(duration)?;
    let marker_index = marker_index_or_raise(project, track_id, marker_id)?;
    if project.markers[marker_index].duration == Some(duration) {
        return Ok(project.markers[marker_index].clone());
    }
    project.markers[marker_index].duration = Some(duration);
    let marker = project.markers[marker_index].clone();
    mark_dependents_stale(project, track_id, "");
    Ok(marker)
}

pub fn delete_editable_marker(
    project: &mut ProjectDocument,
    track_id: &str,
    marker_id: &str,
) -> Result<bool, MarkerError> {
    editable_track_or_raise(project, track_id)?;
    let before = project.markers.len();
    project
        .markers
        .retain(|marker| !(marker.track_id == track_id && marker.id == marker_id));
    let deleted = project.markers.len() != before;
    if deleted {
        mark_dependents_stale(project, track_id, "");
    }
    Ok(deleted)
}

fn editable_track_or_raise<'a>(
    project: &'a ProjectDocument,
    track_id: &str,
) -> Result<&'a Track, MarkerError> {
    let track = find_track(project, track_id)
        .ok_or_else(|| MarkerError::TrackNotFound(track_id.to_string()))?;
    if track.track_type != TrackType::Editable {
        return Err(MarkerError::NotEditableTrack);
    }
    Ok(track)
}

fn marker_index_or_raise(
    project: &ProjectDocument,
    track_id: &str,
    marker_id: &str,
) -> Result<usize, MarkerError> {
    project
        .markers
        .iter()
        .position(|marker| marker.track_id == track_id && marker.id == marker_id)
        .ok_or_else(|| MarkerError::MarkerNotFound {
            track_id: track_id.to_string(),
            marker_id: marker_id.to_string(),
        })
}

fn marker_indices_or_raise(
    project: &ProjectDocument,
    track_id: &str,
    marker_ids: &[String],
) -> Result<Vec<usize>, MarkerError> {
    marker_ids
        .iter()
        .map(|marker_id| marker_index_or_raise(project, track_id, marker_id))
        .collect()
}

fn apply_marker_fields(
    marker: &mut Marker,
    timestamp: f64,
    duration: Option<f64>,
    label: &str,
    category: &str,
    color: &str,
) -> bool {
    let mut changed = false;
    if marker.timestamp != timestamp {
        marker.timestamp = timestamp;
        changed = true;
    }
    if marker.duration != duration {
        marker.duration = duration;
        changed = true;
    }
    if marker.label != label {
        marker.label = label.to_string();
        changed = true;
    }
    if marker.category != category {
        marker.category = category.to_string();
        changed = true;
    }
    if set_marker_color(marker, color) {
        changed = true;
    }
    changed
}

fn apply_bulk_marker_fields(marker: &mut Marker, label: &str, category: &str, color: &str) -> bool {
    let mut changed = false;
    if marker.label != label {
        marker.label = label.to_string();
        changed = true;
    }
    if marker.category != category {
        marker.category = category.to_string();
        changed = true;
    }
    if set_marker_color(marker, color) {
        changed = true;
    }
    changed
}

fn set_marker_color(marker: &mut Marker, color: &str) -> bool {
    let current = marker.metadata.get("color").and_then(Value::as_str);
    if current == Some(color) {
        return false;
    }
    marker
        .metadata
        .insert("color".to_string(), Value::String(color.to_string()));
    true
}

fn finite_marker_timestamp(timestamp: f64) -> Result<f64, MarkerError> {
    if !timestamp.is_finite() {
        return Err(MarkerError::TimestampNotFinite);
    }
    Ok(timestamp)
}

fn finite_movable_marker_timestamp(timestamp: f64) -> Result<f64, MarkerError> {
    if !timestamp.is_finite() {
        return Err(MarkerError::MoveNotFinite);
    }
    Ok(timestamp)
}

fn finite_marker_delta(delta: f64) -> Result<f64, MarkerError> {
    if !delta.is_finite() {
        return Err(MarkerError::DeltaNotFinite);
    }
    Ok(delta)
}

fn optional_marker_duration(duration: Option<f64>) -> Result<Option<f64>, MarkerError> {
    duration.map(finite_marker_duration).transpose()
}

fn finite_marker_duration(duration: f64) -> Result<f64, MarkerError> {
    if !duration.is_finite() {
        return Err(MarkerError::DurationNotFinite);
    }
    if duration < 0.0 {
        return Err(MarkerError::NegativeDuration);
    }
    Ok(duration)
}

fn normalize_marker_color(color: &str) -> Result<String, MarkerError> {
    let value = if color.is_empty() {
        DEFAULT_MARKER_COLOR.to_string()
    } else {
        color.trim().to_lowercase()
    };
    if MARKER_COLOR_KEYS.contains(&value.as_str()) {
        return Ok(value);
    }
    Err(MarkerError::InvalidColor(MARKER_COLOR_KEYS.join(", ")))
}

fn normalize_marker_category(category: &str) -> String {
    if category.is_empty() {
        "cue".to_string()
    } else {
        category.to_string()
    }
}

fn json_object(values: impl IntoIterator<Item = (&'static str, Value)>) -> JsonObject {
    values
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

fn new_id(prefix: &str) -> String {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}_{id:012x}")
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{
        add_editable_marker, bulk_update_editable_markers, create_manual_editable_track,
        delete_editable_marker, move_editable_markers, resize_editable_marker,
        update_editable_marker, BulkMarkerUpdate, EditableMarkerInput, MarkerUpdate,
    };
    use crate::project::{
        AudioAsset, JsonObject, Marker, ProjectDocument, ResultState, Track, TrackType,
    };

    #[test]
    fn markers_create_manual_editable_track_uses_resolved_source_track() {
        let mut project = project_with_generated_track();

        let manual =
            create_manual_editable_track(&mut project, "track_generated", "Manual Cues").unwrap();

        assert_eq!(manual.track_type, TrackType::Editable);
        assert_eq!(manual.input_track_ids, ["track_source"]);
        assert_eq!(manual.result_state, ResultState::Complete);
        assert_eq!(manual.provenance["manual_track"], json!(true));
        assert_eq!(manual.provenance["created_by"], json!("user"));
    }

    #[test]
    fn markers_create_manual_editable_track_rejects_missing_source_context() {
        let mut project = ProjectDocument::new("project_1", "Demo");

        let err = create_manual_editable_track(&mut project, "", "Manual Cues").unwrap_err();

        assert!(err.to_string().contains("source audio"));
    }

    #[test]
    fn markers_add_rejects_generated_track_and_non_finite_timestamp() {
        let mut project = project_with_generated_track();

        let generated_err = add_editable_marker(
            &mut project,
            "track_generated",
            EditableMarkerInput::cue(1.0, "Cue"),
        )
        .unwrap_err();
        project
            .tracks
            .push(editable_track("track_edit", "track_generated"));
        let timestamp_err = add_editable_marker(
            &mut project,
            "track_edit",
            EditableMarkerInput::cue(f64::NAN, "Cue"),
        )
        .unwrap_err();

        assert!(generated_err.to_string().contains("editable track"));
        assert!(timestamp_err.to_string().contains("finite"));
    }

    #[test]
    fn markers_add_and_delete_marker_marks_downstream_stale() {
        let mut project = project_with_editable_track();
        project.tracks.push(generated_track(
            "track_downstream",
            "track_edit",
            ResultState::Complete,
        ));

        let marker = add_editable_marker(
            &mut project,
            "track_edit",
            EditableMarkerInput::cue(1.25, "Cue"),
        )
        .unwrap();
        assert_eq!(
            track_state(&project, "track_downstream"),
            ResultState::Stale
        );
        project
            .tracks
            .iter_mut()
            .find(|track| track.id == "track_downstream")
            .unwrap()
            .result_state = ResultState::Complete;

        let deleted = delete_editable_marker(&mut project, "track_edit", &marker.id).unwrap();

        assert!(deleted);
        assert_eq!(
            track_state(&project, "track_downstream"),
            ResultState::Stale
        );
        assert!(!project.markers.iter().any(|item| item.id == marker.id));
    }

    #[test]
    fn markers_move_editable_markers_is_atomic_for_invalid_results() {
        let mut project = project_with_editable_track();
        let first = add_editable_marker(
            &mut project,
            "track_edit",
            EditableMarkerInput::cue(0.25, "First"),
        )
        .unwrap();
        let second = add_editable_marker(
            &mut project,
            "track_edit",
            EditableMarkerInput::cue(1.25, "Second"),
        )
        .unwrap();

        let err = move_editable_markers(
            &mut project,
            "track_edit",
            &[first.id.clone(), second.id.clone()],
            -0.5,
        )
        .unwrap_err();

        assert!(err.to_string().contains("negative timestamp"));
        assert_eq!(marker_timestamp(&project, &first.id), 0.25);
        assert_eq!(marker_timestamp(&project, &second.id), 1.25);
    }

    #[test]
    fn markers_move_noop_does_not_mark_downstream_stale() {
        let mut project = project_with_editable_track();
        let marker = add_editable_marker(
            &mut project,
            "track_edit",
            EditableMarkerInput::cue(0.25, "Cue"),
        )
        .unwrap();
        project.tracks.push(generated_track(
            "track_downstream",
            "track_edit",
            ResultState::Complete,
        ));

        let moved = move_editable_markers(&mut project, "track_edit", &[marker.id], 0.0).unwrap();

        assert_eq!(moved.len(), 1);
        assert_eq!(
            track_state(&project, "track_downstream"),
            ResultState::Complete
        );
    }

    #[test]
    fn markers_resize_sets_duration_and_rejects_negative_duration() {
        let mut project = project_with_editable_track();
        let marker = add_editable_marker(
            &mut project,
            "track_edit",
            EditableMarkerInput::cue(0.25, "Cue"),
        )
        .unwrap();

        resize_editable_marker(&mut project, "track_edit", &marker.id, 1.5).unwrap();
        let err = resize_editable_marker(&mut project, "track_edit", &marker.id, -0.1).unwrap_err();

        assert!(err.to_string().contains("duration"));
        assert_eq!(marker_duration(&project, &marker.id), Some(1.5));
    }

    #[test]
    fn markers_update_and_bulk_update_metadata_fields() {
        let mut project = project_with_editable_track();
        let first = add_editable_marker(
            &mut project,
            "track_edit",
            EditableMarkerInput::cue(1.0, "A"),
        )
        .unwrap();
        let second = add_editable_marker(
            &mut project,
            "track_edit",
            EditableMarkerInput::cue(2.0, "B"),
        )
        .unwrap();

        update_editable_marker(
            &mut project,
            "track_edit",
            &first.id,
            MarkerUpdate {
                timestamp: 1.5,
                duration: Some(0.25),
                label: "Hit".to_string(),
                category: "accent".to_string(),
                color: "amber".to_string(),
            },
        )
        .unwrap();
        let updated = bulk_update_editable_markers(
            &mut project,
            "track_edit",
            &[second.id.clone()],
            BulkMarkerUpdate {
                label: "Scene".to_string(),
                category: "scene".to_string(),
                color: "blue".to_string(),
            },
        )
        .unwrap();

        assert_eq!(updated, 1);
        assert_eq!(
            marker_by_id(&project, &first.id).metadata["color"],
            json!("amber")
        );
        assert_eq!(marker_by_id(&project, &second.id).label, "Scene");
        assert_eq!(
            marker_by_id(&project, &second.id).metadata["color"],
            json!("blue")
        );
    }

    fn project_with_generated_track() -> ProjectDocument {
        let mut project = project_with_source();
        project.tracks.push(generated_track(
            "track_generated",
            "track_source",
            ResultState::Complete,
        ));
        project
    }

    fn project_with_editable_track() -> ProjectDocument {
        let mut project = project_with_generated_track();
        project
            .tracks
            .push(editable_track("track_edit", "track_generated"));
        project
    }

    fn project_with_source() -> ProjectDocument {
        let mut project = ProjectDocument::new("project_1", "Demo");
        project.audio_assets.push(AudioAsset {
            id: "asset_source".to_string(),
            path: "/fixtures/audio/source.wav".to_string(),
            duration: 12.0,
            sample_rate: 44_100,
            channels: 2,
            fingerprint: "fingerprint".to_string(),
            import_status: "online".to_string(),
            relink_hint: String::new(),
        });
        project.tracks.push(Track {
            id: "track_source".to_string(),
            track_type: TrackType::Source,
            name: "Source".to_string(),
            input_track_ids: Vec::new(),
            transform_id: String::new(),
            transform_params: JsonObject::new(),
            transform_version: String::new(),
            output_schema: String::new(),
            dependency_hash: String::new(),
            result_state: ResultState::Complete,
            cache_refs: Vec::new(),
            provenance: object(json!({ "asset_id": "asset_source" })),
            error: String::new(),
        });
        project
    }

    fn generated_track(id: &str, parent_id: &str, result_state: ResultState) -> Track {
        Track {
            id: id.to_string(),
            track_type: TrackType::Generated,
            name: id.to_string(),
            input_track_ids: vec![parent_id.to_string()],
            transform_id: "markers.fixed_interval".to_string(),
            transform_params: JsonObject::new(),
            transform_version: "1".to_string(),
            output_schema: "markers.v1".to_string(),
            dependency_hash: format!("dep_{id}"),
            result_state,
            cache_refs: Vec::new(),
            provenance: JsonObject::new(),
            error: String::new(),
        }
    }

    fn editable_track(id: &str, parent_id: &str) -> Track {
        Track {
            id: id.to_string(),
            track_type: TrackType::Editable,
            name: id.to_string(),
            input_track_ids: vec![parent_id.to_string()],
            transform_id: String::new(),
            transform_params: JsonObject::new(),
            transform_version: String::new(),
            output_schema: String::new(),
            dependency_hash: String::new(),
            result_state: ResultState::Complete,
            cache_refs: Vec::new(),
            provenance: object(json!({ "source_track_id": parent_id })),
            error: String::new(),
        }
    }

    fn track_state(project: &ProjectDocument, track_id: &str) -> ResultState {
        project
            .tracks
            .iter()
            .find(|track| track.id == track_id)
            .map(|track| track.result_state)
            .unwrap()
    }

    fn marker_by_id<'a>(project: &'a ProjectDocument, marker_id: &str) -> &'a Marker {
        project
            .markers
            .iter()
            .find(|marker| marker.id == marker_id)
            .unwrap()
    }

    fn marker_timestamp(project: &ProjectDocument, marker_id: &str) -> f64 {
        marker_by_id(project, marker_id).timestamp
    }

    fn marker_duration(project: &ProjectDocument, marker_id: &str) -> Option<f64> {
        marker_by_id(project, marker_id).duration
    }

    fn object(value: Value) -> JsonObject {
        value.as_object().cloned().unwrap()
    }
}
