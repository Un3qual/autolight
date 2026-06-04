use std::collections::BTreeSet;

use autolight_core::cache::{track_dependency_hash, track_dependency_inputs};
use autolight_core::graph::find_track;
use autolight_core::project::{JsonObject, ProjectDocument, ResultState, Track, TrackType};
use autolight_core::transforms::TransformSpec;
use serde_json::Value;

pub(super) fn parse_params(params_json: &str) -> Result<JsonObject, String> {
    if params_json.trim().is_empty() {
        return Ok(JsonObject::default());
    }
    let value: Value = serde_json::from_str(params_json).map_err(|error| error.to_string())?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| "transform params must be a JSON object".to_string())
}

pub(super) fn dependency_hash_for_new_track(
    project: &ProjectDocument,
    parent_track_id: &str,
    transform_id: &str,
    version: &str,
    params: &JsonObject,
) -> Result<String, String> {
    let parent = find_track(project, parent_track_id)
        .ok_or_else(|| format!("track not found: {parent_track_id}"))?;
    let input_refs = track_dependency_inputs(project, parent).map_err(|error| error.to_string())?;
    track_dependency_hash(&input_refs, transform_id, version, params)
        .map_err(|error| error.to_string())
}

pub(super) fn parent_compatibility_error(parent: &Track, spec: &TransformSpec) -> String {
    if spec.is_audio_input() {
        match parent.track_type {
            TrackType::Editable => "editable track has no source audio context".to_string(),
            _ => "parent track has no valid audio artifact".to_string(),
        }
    } else {
        "parent track is not compatible with transform".to_string()
    }
}

pub(super) fn track_inputs_are_complete(project: &ProjectDocument, track: &Track) -> bool {
    track.input_track_ids.iter().all(|input_id| {
        find_track(project, input_id)
            .is_some_and(|input| input.result_state == ResultState::Complete)
    })
}

pub(super) fn restore_audio_dependency_dependents(project: &mut ProjectDocument) -> usize {
    let mut restored = 0;
    loop {
        let restorable_ids = project
            .tracks
            .iter()
            .filter(|track| track.track_type != TrackType::Source)
            .filter(|track| {
                track.result_state == ResultState::Stale && is_audio_dependency_error(&track.error)
            })
            .filter(|track| track_inputs_are_complete(project, track))
            .map(|track| track.id.clone())
            .collect::<Vec<_>>();
        if restorable_ids.is_empty() {
            break;
        }
        for track_id in restorable_ids {
            if let Some(track) = project.tracks.iter_mut().find(|track| track.id == track_id) {
                track.result_state = ResultState::Complete;
                track.error.clear();
                restored += 1;
            }
        }
    }
    restored
}

pub(super) fn clear_waveform_provenance(track: &mut Track) {
    track.provenance.remove("waveform_payload");
    track.provenance.remove("waveform_samples");
    track.provenance.remove("waveform_duration_seconds");
    track.provenance.remove("visible_waveform");
}

pub(super) fn is_audio_dependency_error(error: &str) -> bool {
    error.starts_with("input audio asset offline:")
        || error.starts_with("input audio asset modified:")
}

pub(super) fn latest_active_job_id(project: &ProjectDocument, track_id: &str) -> Option<String> {
    project
        .job_runs
        .iter()
        .rev()
        .find(|run| {
            run.track_id == track_id
                && matches!(run.state, ResultState::Pending | ResultState::Running)
        })
        .map(|run| run.id.clone())
}

pub(super) fn expanded_track_ids_from_project(
    project: &ProjectDocument,
) -> Option<BTreeSet<String>> {
    let values = project
        .ui_state
        .get("expanded_track_ids")?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>();
    Some(values)
}

pub(super) fn selected_track_id_from_project(project: &ProjectDocument) -> String {
    let restored = project
        .ui_state
        .get("timeline")
        .and_then(|timeline| timeline.get("selected_track_id"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !restored.is_empty() && find_track(project, restored).is_some() {
        return restored.to_string();
    }
    project
        .tracks
        .first()
        .map(|track| track.id.clone())
        .unwrap_or_default()
}
