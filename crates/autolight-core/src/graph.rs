use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde_json::Value;
use thiserror::Error;

use crate::project::{ProjectDocument, ResultState, Track, TrackType};

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("duplicate audio asset id")]
    DuplicateAudioAssetId,
    #[error("duplicate track id")]
    DuplicateTrackId,
    #[error("duplicate cache entry id")]
    DuplicateCacheEntryId,
    #[error("duplicate marker id")]
    DuplicateMarkerId,
    #[error("duplicate job run id")]
    DuplicateJobRunId,
    #[error("source tracks cannot have inputs")]
    SourceTrackHasInputs,
    #[error("source track references missing audio asset: {0}")]
    SourceTrackMissingAudioAsset(String),
    #[error("generated tracks must have exactly one input")]
    GeneratedTrackInputCount,
    #[error("editable tracks must have exactly one input")]
    EditableTrackInputCount,
    #[error("missing input track: {0}")]
    MissingInputTrack(String),
    #[error("track cache ref not found: {0}")]
    TrackCacheRefNotFound(String),
    #[error("marker references missing track: {0}")]
    MarkerMissingTrack(String),
    #[error("job run references missing track: {0}")]
    JobRunMissingTrack(String),
    #[error("job run cache ref not found: {0}")]
    JobRunCacheRefNotFound(String),
    #[error("cycle detected in track graph: {0}")]
    Cycle(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRow {
    pub track_id: String,
    pub depth: usize,
    pub parent_track_id: String,
    pub has_children: bool,
    pub expanded: bool,
    pub child_count: usize,
    pub visible_child_state_summary: String,
    pub tree_error: String,
}

pub fn validate_graph(project: &ProjectDocument) -> Result<(), GraphError> {
    let audio_asset_ids = unique_ids(
        project.audio_assets.iter().map(|asset| asset.id.as_str()),
        GraphError::DuplicateAudioAssetId,
    )?;
    let track_ids = unique_ids(
        project.tracks.iter().map(|track| track.id.as_str()),
        GraphError::DuplicateTrackId,
    )?;
    let cache_entry_ids = unique_ids(
        project.cache_entries.iter().map(|entry| entry.id.as_str()),
        GraphError::DuplicateCacheEntryId,
    )?;
    unique_ids(
        project.markers.iter().map(|marker| marker.id.as_str()),
        GraphError::DuplicateMarkerId,
    )?;
    unique_ids(
        project.job_runs.iter().map(|run| run.id.as_str()),
        GraphError::DuplicateJobRunId,
    )?;

    for track in &project.tracks {
        match track.track_type {
            TrackType::Source => validate_source_track(track, &audio_asset_ids)?,
            TrackType::Generated => {
                if track.input_track_ids.len() != 1 {
                    return Err(GraphError::GeneratedTrackInputCount);
                }
            }
            TrackType::Editable => {
                if track.input_track_ids.len() != 1 {
                    return Err(GraphError::EditableTrackInputCount);
                }
            }
        }

        for input_id in &track.input_track_ids {
            if !track_ids.contains(input_id) {
                return Err(GraphError::MissingInputTrack(input_id.clone()));
            }
        }
        for cache_ref in &track.cache_refs {
            if !cache_entry_ids.contains(cache_ref) {
                return Err(GraphError::TrackCacheRefNotFound(cache_ref.clone()));
            }
        }
    }

    for marker in &project.markers {
        if !track_ids.contains(&marker.track_id) {
            return Err(GraphError::MarkerMissingTrack(marker.track_id.clone()));
        }
    }

    for run in &project.job_runs {
        if !track_ids.contains(&run.track_id) {
            return Err(GraphError::JobRunMissingTrack(run.track_id.clone()));
        }
        for cache_ref in &run.produced_cache_refs {
            if !cache_entry_ids.contains(cache_ref) {
                return Err(GraphError::JobRunCacheRefNotFound(cache_ref.clone()));
            }
        }
    }

    validate_acyclic(project)
}

pub fn find_track<'a>(project: &'a ProjectDocument, track_id: &str) -> Option<&'a Track> {
    project.tracks.iter().find(|track| track.id == track_id)
}

pub fn find_track_mut<'a>(
    project: &'a mut ProjectDocument,
    track_id: &str,
) -> Option<&'a mut Track> {
    project.tracks.iter_mut().find(|track| track.id == track_id)
}

pub fn mark_dependents_stale(project: &mut ProjectDocument, changed_track_id: &str, error: &str) {
    let mut stale_ids = BTreeSet::from([changed_track_id.to_string()]);
    let mut changed = true;
    while changed {
        changed = false;
        for track in &mut project.tracks {
            if track.track_type == TrackType::Source || stale_ids.contains(&track.id) {
                continue;
            }
            if track
                .input_track_ids
                .iter()
                .any(|input_id| stale_ids.contains(input_id))
            {
                let was_complete = track.result_state == ResultState::Complete;
                track.result_state = ResultState::Stale;
                if !error.is_empty() && (was_complete || is_audio_dependency_error(&track.error)) {
                    track.error = error.to_string();
                }
                stale_ids.insert(track.id.clone());
                changed = true;
            }
        }
    }
}

pub fn source_track_id_for_context(project: &ProjectDocument, track_id: &str) -> Option<String> {
    let tracks_by_id = tracks_by_id(project);
    let mut stack = VecDeque::from([track_id]);
    let mut visited = BTreeSet::new();

    while let Some(current_id) = stack.pop_back() {
        if !visited.insert(current_id.to_string()) {
            continue;
        }
        let track = tracks_by_id.get(current_id)?;
        if track.track_type == TrackType::Source {
            return Some(track.id.clone());
        }
        for input_id in &track.input_track_ids {
            stack.push_back(input_id);
        }
    }

    None
}

pub fn default_expanded_track_ids(project: &ProjectDocument) -> BTreeSet<String> {
    let known_ids: BTreeSet<&str> = project
        .tracks
        .iter()
        .map(|track| track.id.as_str())
        .collect();
    project
        .tracks
        .iter()
        .filter_map(|track| track.input_track_ids.first())
        .filter(|track_id| known_ids.contains(track_id.as_str()))
        .cloned()
        .collect()
}

pub fn project_tree(
    project: &ProjectDocument,
    expanded_track_ids: &BTreeSet<String>,
) -> Vec<TreeRow> {
    let projection = TreeProjection::new(project, expanded_track_ids);
    projection.rows
}

struct TreeProjection<'a> {
    project: &'a ProjectDocument,
    expanded_track_ids: &'a BTreeSet<String>,
    tracks_by_id: BTreeMap<&'a str, usize>,
    children_by_track: BTreeMap<&'a str, Vec<usize>>,
    parents_by_track: BTreeMap<&'a str, &'a str>,
    tree_errors: BTreeMap<&'a str, String>,
    child_state_summaries: BTreeMap<&'a str, String>,
    projected_ids: BTreeSet<&'a str>,
    rows: Vec<TreeRow>,
}

impl<'a> TreeProjection<'a> {
    fn new(project: &'a ProjectDocument, expanded_track_ids: &'a BTreeSet<String>) -> Self {
        let mut projection = Self {
            project,
            expanded_track_ids,
            tracks_by_id: BTreeMap::new(),
            children_by_track: BTreeMap::new(),
            parents_by_track: BTreeMap::new(),
            tree_errors: BTreeMap::new(),
            child_state_summaries: BTreeMap::new(),
            projected_ids: BTreeSet::new(),
            rows: Vec::new(),
        };
        projection.index_tracks();
        projection.compute_child_state_summaries();
        projection.append_roots();
        projection.append_cycle_fallback_rows();
        projection
    }

    fn index_tracks(&mut self) {
        for (index, track) in self.project.tracks.iter().enumerate() {
            self.tracks_by_id.insert(track.id.as_str(), index);
        }
        for (index, track) in self.project.tracks.iter().enumerate() {
            let Some(parent_id) = track.input_track_ids.first().map(String::as_str) else {
                continue;
            };
            if self.tracks_by_id.contains_key(parent_id) {
                self.children_by_track
                    .entry(parent_id)
                    .or_default()
                    .push(index);
                self.parents_by_track.insert(track.id.as_str(), parent_id);
            } else {
                self.tree_errors
                    .insert(track.id.as_str(), format!("missing parent: {parent_id}"));
            }
        }
    }

    fn append_roots(&mut self) {
        for index in 0..self.project.tracks.len() {
            let track = &self.project.tracks[index];
            if self.parents_by_track.contains_key(track.id.as_str()) {
                continue;
            }
            self.append_tree_row(index, 0, &mut BTreeSet::new());
        }
    }

    fn append_cycle_fallback_rows(&mut self) {
        for index in 0..self.project.tracks.len() {
            let track = &self.project.tracks[index];
            if self.projected_ids.contains(track.id.as_str())
                || !self.track_has_parent_cycle(track.id.as_str())
            {
                continue;
            }
            self.tree_errors
                .insert(track.id.as_str(), "cycle detected".to_string());
            self.append_tree_row(index, 0, &mut BTreeSet::new());
        }
    }

    fn append_tree_row(
        &mut self,
        track_index: usize,
        depth: usize,
        active_path: &mut BTreeSet<&'a str>,
    ) {
        let track = &self.project.tracks[track_index];
        if active_path.contains(track.id.as_str()) {
            self.tree_errors
                .insert(track.id.as_str(), "cycle detected".to_string());
            return;
        }
        if !self.projected_ids.insert(track.id.as_str()) {
            return;
        }

        let children = self
            .children_by_track
            .get(track.id.as_str())
            .cloned()
            .unwrap_or_default();
        let expanded = self.expanded_track_ids.contains(&track.id);
        self.rows.push(TreeRow {
            track_id: track.id.clone(),
            depth,
            parent_track_id: self
                .parents_by_track
                .get(track.id.as_str())
                .copied()
                .unwrap_or("")
                .to_string(),
            has_children: !children.is_empty(),
            expanded,
            child_count: children.len(),
            visible_child_state_summary: self
                .child_state_summaries
                .get(track.id.as_str())
                .cloned()
                .unwrap_or_default(),
            tree_error: self
                .tree_errors
                .get(track.id.as_str())
                .cloned()
                .unwrap_or_default(),
        });

        if !expanded {
            return;
        }

        active_path.insert(track.id.as_str());
        for child_index in children {
            self.append_tree_row(child_index, depth + 1, active_path);
        }
        active_path.remove(track.id.as_str());
    }

    fn compute_child_state_summaries(&mut self) {
        let mut counts_by_track = BTreeMap::new();
        for track in &self.project.tracks {
            let counts = self.child_state_counts(
                track.id.as_str(),
                &mut counts_by_track,
                &mut BTreeSet::new(),
            );
            if !counts.is_empty() {
                self.child_state_summaries
                    .insert(track.id.as_str(), format_child_state_counts(counts));
            }
        }
    }

    fn child_state_counts(
        &self,
        track_id: &'a str,
        counts_by_track: &mut BTreeMap<&'a str, BTreeMap<&'static str, usize>>,
        active_path: &mut BTreeSet<&'a str>,
    ) -> BTreeMap<&'static str, usize> {
        if let Some(counts) = counts_by_track.get(track_id) {
            return counts.clone();
        }
        if !active_path.insert(track_id) {
            return BTreeMap::new();
        }

        let mut counts = BTreeMap::new();
        if let Some(children) = self.children_by_track.get(track_id) {
            for child_index in children {
                let child = &self.project.tracks[*child_index];
                if child.result_state != ResultState::Complete {
                    *counts.entry(child.result_state.as_str()).or_default() += 1;
                }
                for (state, count) in
                    self.child_state_counts(child.id.as_str(), counts_by_track, active_path)
                {
                    *counts.entry(state).or_default() += count;
                }
            }
        }

        active_path.remove(track_id);
        counts_by_track.insert(track_id, counts.clone());
        counts
    }

    fn track_has_parent_cycle(&self, track_id: &str) -> bool {
        let mut seen = BTreeSet::new();
        let mut current_track_id = track_id;

        while !current_track_id.is_empty() {
            if !seen.insert(current_track_id) {
                return true;
            }
            let Some(track_index) = self.tracks_by_id.get(current_track_id).copied() else {
                return false;
            };
            let Some(parent_id) = self.project.tracks[track_index]
                .input_track_ids
                .first()
                .map(String::as_str)
            else {
                return false;
            };
            if !self.tracks_by_id.contains_key(parent_id) {
                return false;
            }
            current_track_id = parent_id;
        }
        false
    }
}

fn format_child_state_counts(counts: BTreeMap<&'static str, usize>) -> String {
    counts
        .into_iter()
        .map(|(state, count)| format!("{state}: {count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn validate_source_track(
    track: &Track,
    audio_asset_ids: &BTreeSet<String>,
) -> Result<(), GraphError> {
    if !track.input_track_ids.is_empty() {
        return Err(GraphError::SourceTrackHasInputs);
    }
    let asset_id = track.provenance.get("asset_id").and_then(Value::as_str);
    if !asset_id.is_some_and(|id| audio_asset_ids.contains(id)) {
        return Err(GraphError::SourceTrackMissingAudioAsset(track.id.clone()));
    }
    Ok(())
}

fn validate_acyclic(project: &ProjectDocument) -> Result<(), GraphError> {
    let tracks_by_id = tracks_by_id(project);
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();

    for track in &project.tracks {
        visit_track(
            track.id.as_str(),
            &tracks_by_id,
            &mut visiting,
            &mut visited,
        )?;
    }
    Ok(())
}

fn visit_track<'a>(
    track_id: &'a str,
    tracks_by_id: &BTreeMap<&'a str, &'a Track>,
    visiting: &mut BTreeSet<&'a str>,
    visited: &mut BTreeSet<&'a str>,
) -> Result<(), GraphError> {
    if visiting.contains(track_id) {
        return Err(GraphError::Cycle(track_id.to_string()));
    }
    if visited.contains(track_id) {
        return Ok(());
    }

    visiting.insert(track_id);
    let Some(track) = tracks_by_id.get(track_id) else {
        return Ok(());
    };
    for input_id in &track.input_track_ids {
        visit_track(input_id, tracks_by_id, visiting, visited)?;
    }
    visiting.remove(track_id);
    visited.insert(track_id);
    Ok(())
}

fn tracks_by_id(project: &ProjectDocument) -> BTreeMap<&str, &Track> {
    project
        .tracks
        .iter()
        .map(|track| (track.id.as_str(), track))
        .collect()
}

fn unique_ids<'a>(
    ids: impl Iterator<Item = &'a str>,
    duplicate_error: GraphError,
) -> Result<BTreeSet<String>, GraphError> {
    let mut unique = BTreeSet::new();
    for id in ids {
        if !unique.insert(id.to_string()) {
            return Err(duplicate_error);
        }
    }
    Ok(unique)
}

fn is_audio_dependency_error(error: &str) -> bool {
    error.starts_with("input audio asset offline:")
        || error.starts_with("input audio asset modified:")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    use serde_json::{json, Value};

    use super::{
        default_expanded_track_ids, mark_dependents_stale, project_tree,
        source_track_id_for_context, validate_graph,
    };
    use crate::project::{
        AudioAsset, CacheEntry, JobRun, JsonObject, Marker, ProjectDocument, ResultState, Track,
        TrackType,
    };

    fn fixture_path(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/projects")
            .join(name)
    }

    #[test]
    fn graph_validate_accepts_schema_fixture_projects() {
        let basic = ProjectDocument::load_path(fixture_path("basic_graph.autolight")).unwrap();
        let tree = ProjectDocument::load_path(fixture_path("tree_analysis.autolight")).unwrap();

        validate_graph(&basic).unwrap();
        validate_graph(&tree).unwrap();
    }

    #[test]
    fn graph_validate_rejects_missing_inputs_and_orphan_references() {
        let mut project = project_with_source();
        project.tracks.push(generated_track(
            "track_missing_input",
            "track_not_here",
            ResultState::Complete,
        ));
        project
            .markers
            .push(marker_on_track("marker_orphan", "track_not_here", 1.0));

        let err = validate_graph(&project).unwrap_err();

        assert!(err
            .to_string()
            .contains("missing input track: track_not_here"));
    }

    #[test]
    fn graph_validate_rejects_cycles() {
        let mut project = ProjectDocument::new("project_cycle", "Cycle");
        project.tracks.extend([
            generated_track("track_a", "track_b", ResultState::Complete),
            generated_track("track_b", "track_a", ResultState::Complete),
        ]);

        let err = validate_graph(&project).unwrap_err();

        assert!(err.to_string().contains("cycle"));
    }

    #[test]
    fn graph_mark_dependents_stale_preserves_editable_markers() {
        let mut project = project_with_source();
        project.tracks.push(generated_track(
            "track_beats",
            "track_source",
            ResultState::Complete,
        ));
        project
            .tracks
            .push(editable_track("track_edit", "track_beats"));
        project.tracks.push(generated_track(
            "track_pitch",
            "track_beats",
            ResultState::Complete,
        ));
        project
            .markers
            .push(marker_on_track("marker_edit", "track_edit", 1.0));

        mark_dependents_stale(&mut project, "track_beats", "");

        assert_eq!(track_state(&project, "track_edit"), ResultState::Stale);
        assert_eq!(track_state(&project, "track_pitch"), ResultState::Stale);
        assert_eq!(
            project
                .markers
                .iter()
                .filter(|marker| marker.track_id == "track_edit")
                .count(),
            1
        );
    }

    #[test]
    fn graph_source_track_id_for_context_resolves_generated_ancestor() {
        let mut project = project_with_source();
        project.tracks.push(generated_track(
            "track_beats",
            "track_source",
            ResultState::Complete,
        ));
        project.tracks.push(generated_track(
            "track_energy",
            "track_beats",
            ResultState::Complete,
        ));

        let source_id = source_track_id_for_context(&project, "track_energy").unwrap();

        assert_eq!(source_id, "track_source");
    }

    #[test]
    fn graph_default_tree_projection_expands_known_parent_tracks() {
        let mut project = project_with_source();
        project.tracks.push(generated_track(
            "track_drums",
            "track_source",
            ResultState::Complete,
        ));
        project.tracks.push(generated_track(
            "track_onsets",
            "track_drums",
            ResultState::Stale,
        ));
        project.tracks.push(generated_track(
            "track_beats",
            "track_source",
            ResultState::Complete,
        ));

        let rows = project_tree(&project, &default_expanded_track_ids(&project));

        assert_eq!(
            rows.iter()
                .map(|row| row.track_id.as_str())
                .collect::<Vec<_>>(),
            ["track_source", "track_drums", "track_onsets", "track_beats"]
        );
        assert_eq!(
            rows.iter().map(|row| row.depth).collect::<Vec<_>>(),
            [0, 1, 2, 1]
        );
        assert_eq!(rows[0].child_count, 2);
        assert_eq!(rows[1].visible_child_state_summary, "stale: 1");
    }

    #[test]
    fn graph_tree_projection_surfaces_cycle_as_problem_root_row() {
        let mut project = ProjectDocument::new("project_cycle", "Cycle");
        project.tracks.extend([
            generated_track("track_a", "track_b", ResultState::Complete),
            generated_track("track_b", "track_a", ResultState::Complete),
        ]);

        let rows = project_tree(&project, &BTreeSet::from(["track_a".to_string()]));

        assert!(rows.iter().any(|row| row.tree_error == "cycle detected"));
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
        project.tracks.push(source_track());
        project
    }

    fn source_track() -> Track {
        Track {
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
        }
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

    fn marker_on_track(id: &str, track_id: &str, timestamp: f64) -> Marker {
        Marker {
            id: id.to_string(),
            track_id: track_id.to_string(),
            timestamp,
            duration: None,
            label: String::new(),
            category: String::new(),
            confidence: None,
            tags: Vec::new(),
            source_transform: String::new(),
            source_marker_ids: Vec::new(),
            metadata: JsonObject::new(),
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

    fn object(value: Value) -> JsonObject {
        value.as_object().cloned().unwrap()
    }

    #[allow(dead_code)]
    fn cache_entry(id: &str) -> CacheEntry {
        CacheEntry {
            id: id.to_string(),
            dependency_hash: "dep".to_string(),
            artifact_kind: "markers".to_string(),
            path: "cache/markers.json".to_string(),
            created_at: String::new(),
            transform_version: "1".to_string(),
            size_bytes: 0,
            payload_digest: String::new(),
            validation_status: "valid".to_string(),
        }
    }

    #[allow(dead_code)]
    fn job_run(id: &str, track_id: &str) -> JobRun {
        JobRun {
            id: id.to_string(),
            track_id: track_id.to_string(),
            transform_id: "markers.fixed_interval".to_string(),
            parameters_hash: "dep".to_string(),
            parameters: JsonObject::new(),
            state: ResultState::Complete,
            progress: 1.0,
            started_at: String::new(),
            completed_at: String::new(),
            error: String::new(),
            produced_cache_refs: Vec::new(),
        }
    }
}
