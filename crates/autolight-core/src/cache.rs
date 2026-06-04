use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::graph::mark_dependents_stale;
use crate::project::{CacheEntry, JsonObject, ProjectDocument, ResultState, Track, TrackType};

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("failed to serialize canonical json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid artifact kind: {0}")]
    InvalidArtifactKind(String),
}

pub fn canonical_hash(value: &Value) -> Result<String, CacheError> {
    let canonical = serde_json::to_vec(value)?;
    Ok(sha256_hex(&canonical))
}

pub fn track_dependency_hash(
    input_cache_refs: &[String],
    transform_id: &str,
    transform_version: &str,
    params: &JsonObject,
) -> Result<String, CacheError> {
    canonical_hash(&json!({
        "input_cache_refs": input_cache_refs,
        "transform_id": transform_id,
        "transform_version": transform_version,
        "params": params,
    }))
}

pub fn track_dependency_inputs(
    project: &ProjectDocument,
    track: &Track,
) -> Result<Vec<String>, CacheError> {
    if !track.cache_refs.is_empty() {
        return Ok(track.cache_refs.clone());
    }
    Ok(vec![format!(
        "track:{}:{}",
        track.id,
        track_content_hash(project, track)?
    )])
}

pub fn cache_entry_for_bytes(
    artifact_kind: &str,
    dependency_hash: &str,
    payload: &[u8],
    transform_version: &str,
    created_at: impl Into<String>,
) -> Result<CacheEntry, CacheError> {
    validate_artifact_kind(artifact_kind)?;
    let payload_digest = sha256_hex(payload);
    let entry_id = canonical_hash(&json!({
        "kind": artifact_kind,
        "dependency": dependency_hash,
        "payload_digest": payload_digest,
    }))?;

    Ok(CacheEntry {
        id: entry_id.clone(),
        dependency_hash: dependency_hash.to_string(),
        artifact_kind: artifact_kind.to_string(),
        path: format!("{artifact_kind}/{entry_id}.bin"),
        created_at: created_at.into(),
        transform_version: transform_version.to_string(),
        size_bytes: payload.len() as u64,
        payload_digest,
        validation_status: "valid".to_string(),
    })
}

pub fn cache_entry_matches_payload(entry: &CacheEntry, payload: &[u8]) -> Result<bool, CacheError> {
    if !is_valid_artifact_kind(&entry.artifact_kind) {
        return Ok(false);
    }
    if entry.size_bytes != payload.len() as u64 {
        return Ok(false);
    }
    let payload_digest = sha256_hex(payload);
    let expected_id = canonical_hash(&json!({
        "kind": entry.artifact_kind,
        "dependency": entry.dependency_hash,
        "payload_digest": payload_digest,
    }))?;

    Ok(entry.payload_digest == payload_digest && entry.id == expected_id)
}

pub fn validate_artifact_kind(artifact_kind: &str) -> Result<(), CacheError> {
    if is_valid_artifact_kind(artifact_kind) {
        return Ok(());
    }
    Err(CacheError::InvalidArtifactKind(artifact_kind.to_string()))
}

pub fn is_valid_artifact_kind(artifact_kind: &str) -> bool {
    !artifact_kind.is_empty()
        && artifact_kind
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

pub fn upsert_cache_entry(project: &mut ProjectDocument, entry: CacheEntry) {
    if let Some(existing) = project
        .cache_entries
        .iter_mut()
        .find(|candidate| candidate.id == entry.id)
    {
        *existing = entry;
    } else {
        project.cache_entries.push(entry);
    }
}

pub fn cache_entries_by_id(project: &ProjectDocument) -> BTreeMap<&str, &CacheEntry> {
    project
        .cache_entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect()
}

pub fn artifact_kinds_for_track(project: &ProjectDocument, track: &Track) -> Vec<String> {
    let entries = cache_entries_by_id(project);
    track
        .cache_refs
        .iter()
        .filter_map(|cache_ref| entries.get(cache_ref.as_str()))
        .map(|entry| entry.artifact_kind.clone())
        .collect()
}

pub fn invalid_cache_refs(
    project: &ProjectDocument,
    mut is_entry_valid: impl FnMut(&CacheEntry) -> bool,
) -> Vec<String> {
    let entries = cache_entries_by_id(project);
    let mut invalid = BTreeSet::new();

    for track in &project.tracks {
        for cache_ref in &track.cache_refs {
            match entries.get(cache_ref.as_str()) {
                Some(entry) if is_entry_valid(entry) => {}
                _ => {
                    invalid.insert(cache_ref.clone());
                }
            }
        }
    }

    invalid.into_iter().collect()
}

pub fn mark_invalid_cache_refs_stale(project: &mut ProjectDocument, invalid_refs: &[String]) {
    let invalid_refs = invalid_refs.iter().collect::<BTreeSet<_>>();
    if invalid_refs.is_empty() {
        return;
    }

    let mut affected_track_ids = Vec::default();
    for track in &mut project.tracks {
        if track
            .cache_refs
            .iter()
            .any(|cache_ref| invalid_refs.contains(cache_ref))
        {
            let invalid_ref = track
                .cache_refs
                .iter()
                .find(|cache_ref| invalid_refs.contains(cache_ref))
                .cloned()
                .unwrap_or_default();
            track.result_state = ResultState::Stale;
            track.error = format!("cache artifact missing or invalid: {invalid_ref}");
            affected_track_ids.push(track.id.clone());
        }
    }

    for track_id in affected_track_ids {
        mark_dependents_stale(project, &track_id, "cache artifact missing or invalid");
    }
}

fn sha256_hex(payload: &[u8]) -> String {
    let digest = Sha256::digest(payload);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn track_content_hash(project: &ProjectDocument, track: &Track) -> Result<String, CacheError> {
    let mut markers = project
        .markers
        .iter()
        .filter(|marker| marker.track_id == track.id)
        .collect::<Vec<_>>();
    markers.sort_by(|left, right| {
        left.timestamp
            .total_cmp(&right.timestamp)
            .then_with(|| left.id.cmp(&right.id))
    });

    canonical_hash(&json!({
        "track_id": track.id,
        "track_type": track.track_type.as_str(),
        "input_track_ids": track.input_track_ids,
        "dependency_hash": track.dependency_hash,
        "provenance": track.provenance,
        "markers": markers,
        "audio_asset": source_audio_asset_payload(project, track),
    }))
}

fn source_audio_asset_payload(project: &ProjectDocument, track: &Track) -> Value {
    if track.track_type != TrackType::Source {
        return Value::Null;
    }
    let asset_id = track.provenance.get("asset_id").and_then(Value::as_str);
    let Some(asset) = project
        .audio_assets
        .iter()
        .find(|asset| asset_id.is_some_and(|id| id == asset.id))
    else {
        return json!({
            "asset_id": asset_id.unwrap_or_default(),
            "status": "missing",
        });
    };
    json!({
        "asset_id": asset.id,
        "fingerprint": asset.fingerprint,
        "import_status": asset.import_status,
        "relink_hint": asset.relink_hint,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        artifact_kinds_for_track, cache_entry_for_bytes, cache_entry_matches_payload,
        canonical_hash, invalid_cache_refs, mark_invalid_cache_refs_stale, track_dependency_hash,
        track_dependency_inputs, validate_artifact_kind,
    };
    use crate::project::{
        CacheEntry, JsonObject, Marker, ProjectDocument, ResultState, Track, TrackType,
    };

    #[test]
    fn cache_canonical_hash_is_order_stable() {
        let left = canonical_hash(&json!({"b": 2, "a": 1})).unwrap();
        let right = canonical_hash(&json!({"a": 1, "b": 2})).unwrap();

        assert_eq!(left, right);
    }

    #[test]
    fn cache_track_dependency_hash_includes_inputs_transform_version_and_params() {
        let params = object(json!({"interval": 0.5}));
        let base = track_dependency_hash(&["audio:abc".to_string()], "markers.beats", "1", &params)
            .unwrap();

        assert_ne!(
            base,
            track_dependency_hash(&["audio:def".to_string()], "markers.beats", "1", &params,)
                .unwrap()
        );
        assert_ne!(
            base,
            track_dependency_hash(
                &["audio:abc".to_string()],
                "markers.downbeats",
                "1",
                &params,
            )
            .unwrap()
        );
        assert_ne!(
            base,
            track_dependency_hash(&["audio:abc".to_string()], "markers.beats", "2", &params,)
                .unwrap()
        );
        assert_ne!(
            base,
            track_dependency_hash(
                &["audio:abc".to_string()],
                "markers.beats",
                "1",
                &object(json!({"interval": 1.0})),
            )
            .unwrap()
        );
    }

    #[test]
    fn cache_entry_metadata_uses_payload_digest_identity_and_validates_kind() {
        let entry = cache_entry_for_bytes("markers", "dep_hash", b"[]", "1", "now").unwrap();

        assert_eq!(entry.id.len(), 64);
        assert_eq!(entry.artifact_kind, "markers");
        assert_eq!(entry.size_bytes, 2);
        assert_eq!(entry.transform_version, "1");
        assert_eq!(entry.created_at, "now");
        assert_eq!(entry.path, format!("markers/{}.bin", entry.id));
        assert!(cache_entry_matches_payload(&entry, b"[]").unwrap());
        assert!(!cache_entry_matches_payload(&entry, b"{}").unwrap());

        for artifact_kind in [
            "",
            "../markers",
            "/markers",
            "markers/nested",
            "markers.beats",
        ] {
            assert!(validate_artifact_kind(artifact_kind).is_err());
        }
    }

    #[test]
    fn cache_artifact_kinds_follow_track_cache_refs() {
        let mut project = ProjectDocument::new("project_1", "Demo");
        project.cache_entries.extend([
            cache_entry("cache_waveform", "waveform"),
            cache_entry("cache_energy", "energy"),
        ]);
        project.tracks.push(track_with_cache_refs(
            "track_waveform",
            vec!["cache_waveform".to_string(), "cache_missing".to_string()],
        ));

        let kinds = artifact_kinds_for_track(&project, &project.tracks[0]);

        assert_eq!(kinds, ["waveform"]);
    }

    #[test]
    fn cache_track_dependency_inputs_prefer_cache_refs_then_content_hash() {
        let mut project = ProjectDocument::new("project_1", "Demo");
        project.tracks.push(track_with_cache_refs(
            "track_cached",
            vec!["cache_audio".to_string()],
        ));
        project
            .tracks
            .push(generated_track("track_markers", "track_cached"));

        assert_eq!(
            track_dependency_inputs(&project, &project.tracks[0]).unwrap(),
            ["cache_audio"]
        );
        let initial = track_dependency_inputs(&project, &project.tracks[1]).unwrap();

        project.markers.push(Marker {
            id: "marker_1".to_string(),
            track_id: "track_markers".to_string(),
            timestamp: 1.0,
            duration: None,
            label: "Beat".to_string(),
            category: "timing".to_string(),
            confidence: Some(1.0),
            tags: Vec::default(),
            source_transform: "markers.fixed_interval".to_string(),
            source_marker_ids: Vec::default(),
            metadata: JsonObject::default(),
        });
        let changed = track_dependency_inputs(&project, &project.tracks[1]).unwrap();

        assert!(initial[0].starts_with("track:track_markers:"));
        assert_ne!(initial, changed);
    }

    #[test]
    fn cache_invalid_refs_mark_track_and_dependents_stale() {
        let mut project = ProjectDocument::new("project_1", "Demo");
        project
            .cache_entries
            .push(cache_entry("cache_missing", "stem"));
        project.tracks.extend([
            track_with_cache_refs("track_upstream", vec!["cache_missing".to_string()]),
            generated_track("track_downstream", "track_upstream"),
            editable_track("track_edit", "track_upstream"),
        ]);

        let invalid = invalid_cache_refs(&project, |_| false);
        mark_invalid_cache_refs_stale(&mut project, &invalid);

        assert_eq!(invalid, ["cache_missing"]);
        assert_eq!(track_state(&project, "track_upstream"), ResultState::Stale);
        assert_eq!(
            track_state(&project, "track_downstream"),
            ResultState::Stale
        );
        assert_eq!(track_state(&project, "track_edit"), ResultState::Stale);
        assert!(track_error(&project, "track_upstream").contains("cache artifact"));
    }

    fn cache_entry(id: &str, artifact_kind: &str) -> CacheEntry {
        CacheEntry {
            id: id.to_string(),
            dependency_hash: "dep".to_string(),
            artifact_kind: artifact_kind.to_string(),
            path: format!("{artifact_kind}/{id}.bin"),
            created_at: String::default(),
            transform_version: "1".to_string(),
            size_bytes: 0,
            payload_digest: String::default(),
            validation_status: "valid".to_string(),
        }
    }

    fn track_with_cache_refs(id: &str, cache_refs: Vec<String>) -> Track {
        Track {
            id: id.to_string(),
            track_type: TrackType::Generated,
            name: id.to_string(),
            input_track_ids: Vec::default(),
            transform_id: "stems.vocals_stand_in".to_string(),
            transform_params: JsonObject::default(),
            transform_version: "1".to_string(),
            output_schema: "artifact.stem.v1".to_string(),
            dependency_hash: "dep".to_string(),
            result_state: ResultState::Complete,
            cache_refs,
            provenance: JsonObject::default(),
            error: String::default(),
        }
    }

    fn generated_track(id: &str, parent_id: &str) -> Track {
        Track {
            id: id.to_string(),
            track_type: TrackType::Generated,
            name: id.to_string(),
            input_track_ids: vec![parent_id.to_string()],
            transform_id: "markers.fixed_interval".to_string(),
            transform_params: JsonObject::default(),
            transform_version: "1".to_string(),
            output_schema: "markers.v1".to_string(),
            dependency_hash: "dep_child".to_string(),
            result_state: ResultState::Complete,
            cache_refs: Vec::default(),
            provenance: JsonObject::default(),
            error: String::default(),
        }
    }

    fn editable_track(id: &str, parent_id: &str) -> Track {
        Track {
            id: id.to_string(),
            track_type: TrackType::Editable,
            name: id.to_string(),
            input_track_ids: vec![parent_id.to_string()],
            transform_id: String::default(),
            transform_params: JsonObject::default(),
            transform_version: String::default(),
            output_schema: String::default(),
            dependency_hash: String::default(),
            result_state: ResultState::Complete,
            cache_refs: Vec::default(),
            provenance: JsonObject::default(),
            error: String::default(),
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

    fn track_error<'a>(project: &'a ProjectDocument, track_id: &str) -> &'a str {
        project
            .tracks
            .iter()
            .find(|track| track.id == track_id)
            .map(|track| track.error.as_str())
            .unwrap()
    }

    fn object(value: serde_json::Value) -> JsonObject {
        value.as_object().cloned().unwrap()
    }
}
