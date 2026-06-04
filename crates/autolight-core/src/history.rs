use std::collections::BTreeSet;

use thiserror::Error;

use crate::graph::{find_track, mark_dependents_stale};
use crate::project::{JobRun, Marker, ProjectDocument, Track};

pub trait EditCommand {
    fn undo(&self, project: &mut ProjectDocument) -> Result<(), HistoryError>;
    fn redo(&self, project: &mut ProjectDocument) -> Result<(), HistoryError>;
}

#[derive(Debug, Error)]
pub enum HistoryError {
    #[error("{0}")]
    Obsolete(String),
    #[error("{0}")]
    Command(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarkerSnapshotCommand {
    pub track_id: String,
    pub before: Vec<Marker>,
    pub after: Vec<Marker>,
    pub before_dependents: Vec<DependentTrackSnapshot>,
    pub after_dependents: Vec<DependentTrackSnapshot>,
}

impl MarkerSnapshotCommand {
    pub fn undo(&self, project: &mut ProjectDocument) -> Result<(), HistoryError> {
        self.restore(project, &self.before, &self.before_dependents)
    }

    pub fn redo(&self, project: &mut ProjectDocument) -> Result<(), HistoryError> {
        self.restore(project, &self.after, &self.after_dependents)
    }

    fn restore(
        &self,
        project: &mut ProjectDocument,
        snapshots: &[Marker],
        dependent_snapshots: &[DependentTrackSnapshot],
    ) -> Result<(), HistoryError> {
        if find_track(project, &self.track_id).is_none() {
            return Err(HistoryError::Obsolete(format!(
                "cannot restore markers for missing track: {}",
                self.track_id
            )));
        }
        let affected_ids: BTreeSet<&str> = self
            .before
            .iter()
            .chain(&self.after)
            .map(|marker| marker.id.as_str())
            .collect();
        project.markers.retain(|marker| {
            !(marker.track_id == self.track_id && affected_ids.contains(marker.id.as_str()))
        });
        project.markers.extend(snapshots.iter().cloned());
        mark_dependents_stale(project, &self.track_id, "");
        if !dependent_snapshots.is_empty() {
            restore_dependent_states(project, dependent_snapshots);
        }
        Ok(())
    }
}

impl EditCommand for MarkerSnapshotCommand {
    fn undo(&self, project: &mut ProjectDocument) -> Result<(), HistoryError> {
        MarkerSnapshotCommand::undo(self, project)
    }

    fn redo(&self, project: &mut ProjectDocument) -> Result<(), HistoryError> {
        MarkerSnapshotCommand::redo(self, project)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DependentTrackSnapshot {
    pub track: Track,
    pub index: usize,
    pub markers: Vec<Marker>,
    pub job_runs: Vec<JobRun>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrackSnapshotCommand {
    pub track_id: String,
    pub before: Option<Track>,
    pub after: Option<Track>,
    pub index: usize,
    pub before_markers: Vec<Marker>,
    pub after_markers: Vec<Marker>,
    pub before_job_runs: Vec<JobRun>,
    pub after_job_runs: Vec<JobRun>,
}

impl TrackSnapshotCommand {
    pub fn undo(&self, project: &mut ProjectDocument) -> Result<(), HistoryError> {
        self.restore(
            project,
            self.before.as_ref(),
            &self.before_markers,
            &self.before_job_runs,
        )
    }

    pub fn redo(&self, project: &mut ProjectDocument) -> Result<(), HistoryError> {
        self.restore(
            project,
            self.after.as_ref(),
            &self.after_markers,
            &self.after_job_runs,
        )
    }

    fn restore(
        &self,
        project: &mut ProjectDocument,
        snapshot: Option<&Track>,
        markers: &[Marker],
        job_runs: &[JobRun],
    ) -> Result<(), HistoryError> {
        match snapshot {
            Some(track) => {
                self.replace_track(project, track, markers, job_runs);
                Ok(())
            }
            None => self.remove_track(project),
        }
    }

    fn remove_track(&self, project: &mut ProjectDocument) -> Result<(), HistoryError> {
        if let Some(dependent) = self.dependent_track(project) {
            return Err(HistoryError::Obsolete(format!(
                "cannot remove track with dependent track: {}",
                dependent.id
            )));
        }
        self.discard_track_state(project);
        Ok(())
    }

    fn replace_track(
        &self,
        project: &mut ProjectDocument,
        snapshot: &Track,
        markers: &[Marker],
        job_runs: &[JobRun],
    ) {
        self.discard_track_state(project);
        let insert_at = self.index.min(project.tracks.len());
        project.tracks.insert(insert_at, snapshot.clone());
        project.markers.extend(markers.iter().cloned());
        project.job_runs.extend(job_runs.iter().cloned());
    }

    fn dependent_track<'a>(&self, project: &'a ProjectDocument) -> Option<&'a Track> {
        project.tracks.iter().find(|track| {
            track.id != self.track_id
                && track
                    .input_track_ids
                    .iter()
                    .any(|input_id| input_id == &self.track_id)
        })
    }

    fn discard_track_state(&self, project: &mut ProjectDocument) {
        project.tracks.retain(|track| track.id != self.track_id);
        project
            .markers
            .retain(|marker| marker.track_id != self.track_id);
        project.job_runs.retain(|job| job.track_id != self.track_id);
    }
}

impl EditCommand for TrackSnapshotCommand {
    fn undo(&self, project: &mut ProjectDocument) -> Result<(), HistoryError> {
        TrackSnapshotCommand::undo(self, project)
    }

    fn redo(&self, project: &mut ProjectDocument) -> Result<(), HistoryError> {
        TrackSnapshotCommand::redo(self, project)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectSnapshotCommand {
    pub before: ProjectDocument,
    pub after: ProjectDocument,
}

impl ProjectSnapshotCommand {
    pub fn undo(&self, project: &mut ProjectDocument) -> Result<(), HistoryError> {
        Self::restore(project, &self.before);
        Ok(())
    }

    pub fn redo(&self, project: &mut ProjectDocument) -> Result<(), HistoryError> {
        Self::restore(project, &self.after);
        Ok(())
    }

    fn restore(project: &mut ProjectDocument, snapshot: &ProjectDocument) {
        project.id = snapshot.id.clone();
        project.name = snapshot.name.clone();
        project.schema_version = snapshot.schema_version;
        project.audio_assets = snapshot.audio_assets.clone();
        project.tracks = snapshot.tracks.clone();
        project.markers = snapshot.markers.clone();
        project.job_runs = snapshot.job_runs.clone();
        project.cache_entries = snapshot.cache_entries.clone();
        project.ui_state = snapshot.ui_state.clone();
    }
}

impl EditCommand for ProjectSnapshotCommand {
    fn undo(&self, project: &mut ProjectDocument) -> Result<(), HistoryError> {
        ProjectSnapshotCommand::undo(self, project)
    }

    fn redo(&self, project: &mut ProjectDocument) -> Result<(), HistoryError> {
        ProjectSnapshotCommand::redo(self, project)
    }
}

pub struct EditHistory {
    undo_stack: Vec<Box<dyn EditCommand>>,
    redo_stack: Vec<Box<dyn EditCommand>>,
    clean_undo_depth: Option<usize>,
}

impl EditHistory {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            clean_undo_depth: Some(0),
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn push(&mut self, command: impl EditCommand + 'static) {
        if self
            .clean_undo_depth
            .is_some_and(|depth| !self.redo_stack.is_empty() && depth > self.undo_stack.len())
        {
            self.clean_undo_depth = None;
        }
        self.undo_stack.push(Box::new(command));
        self.redo_stack.clear();
    }

    pub fn undo(&mut self, project: &mut ProjectDocument) -> Result<bool, HistoryError> {
        while let Some(command) = self.undo_stack.pop() {
            match command.undo(project) {
                Ok(()) => {
                    self.redo_stack.push(command);
                    return Ok(true);
                }
                Err(HistoryError::Obsolete(_)) => {
                    self.clean_undo_depth = None;
                    continue;
                }
                Err(err) => {
                    self.undo_stack.push(command);
                    return Err(err);
                }
            }
        }
        Ok(false)
    }

    pub fn redo(&mut self, project: &mut ProjectDocument) -> Result<bool, HistoryError> {
        while let Some(command) = self.redo_stack.pop() {
            match command.redo(project) {
                Ok(()) => {
                    self.undo_stack.push(command);
                    return Ok(true);
                }
                Err(HistoryError::Obsolete(_)) => {
                    self.clean_undo_depth = None;
                    continue;
                }
                Err(err) => {
                    self.redo_stack.push(command);
                    return Err(err);
                }
            }
        }
        Ok(false)
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.clean_undo_depth = Some(0);
    }

    pub fn mark_clean(&mut self) {
        self.clean_undo_depth = Some(self.undo_stack.len());
    }

    pub fn is_clean(&self) -> bool {
        self.clean_undo_depth == Some(self.undo_stack.len())
    }
}

impl Default for EditHistory {
    fn default() -> Self {
        Self::new()
    }
}

fn restore_dependent_states(project: &mut ProjectDocument, snapshots: &[DependentTrackSnapshot]) {
    let track_ids: BTreeSet<&str> = snapshots
        .iter()
        .map(|snapshot| snapshot.track.id.as_str())
        .collect();
    project
        .markers
        .retain(|marker| !track_ids.contains(marker.track_id.as_str()));
    project
        .job_runs
        .retain(|job| !track_ids.contains(job.track_id.as_str()));

    for snapshot in snapshots {
        let insert_at = snapshot.index.min(project.tracks.len());
        if let Some(index) = project
            .tracks
            .iter()
            .position(|track| track.id == snapshot.track.id)
        {
            project.tracks[index] = snapshot.track.clone();
        } else {
            project.tracks.insert(insert_at, snapshot.track.clone());
        }
        project.markers.extend(snapshot.markers.iter().cloned());
        project.job_runs.extend(snapshot.job_runs.iter().cloned());
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{EditHistory, MarkerSnapshotCommand, ProjectSnapshotCommand, TrackSnapshotCommand};
    use crate::markers::{
        add_editable_marker, create_manual_editable_track, update_editable_marker,
        EditableMarkerInput, MarkerUpdate,
    };
    use crate::project::{
        AudioAsset, JsonObject, Marker, ProjectDocument, ResultState, Track, TrackType,
    };

    #[test]
    fn history_undoes_and_redoes_marker_snapshot_command() {
        let mut project = project_with_editable_track();
        let marker = add_editable_marker(
            &mut project,
            "track_edit",
            EditableMarkerInput::cue(1.0, "Cue"),
        )
        .unwrap();
        let before = vec![marker.clone()];
        let updated = update_editable_marker(
            &mut project,
            "track_edit",
            &marker.id,
            MarkerUpdate {
                timestamp: 2.0,
                duration: None,
                label: "Hit".to_string(),
                category: "accent".to_string(),
                color: "amber".to_string(),
            },
        )
        .unwrap();
        let after = vec![updated];
        let mut history = EditHistory::new();
        history.push(MarkerSnapshotCommand {
            track_id: "track_edit".to_string(),
            before,
            after,
            before_dependents: Vec::new(),
            after_dependents: Vec::new(),
        });

        assert!(history.undo(&mut project).unwrap());
        assert_eq!(marker_by_id(&project, &marker.id).timestamp, 1.0);
        assert_eq!(marker_by_id(&project, &marker.id).label, "Cue");

        assert!(history.redo(&mut project).unwrap());
        assert_eq!(marker_by_id(&project, &marker.id).timestamp, 2.0);
        assert_eq!(
            marker_by_id(&project, &marker.id).metadata["color"],
            json!("amber")
        );
    }

    #[test]
    fn history_marker_snapshot_restore_rejects_missing_track() {
        let mut project = project_with_editable_track();
        project.tracks.retain(|track| track.id != "track_edit");
        let command = MarkerSnapshotCommand {
            track_id: "track_edit".to_string(),
            before: Vec::new(),
            after: vec![Marker {
                id: "marker_orphan".to_string(),
                track_id: "track_edit".to_string(),
                timestamp: 1.0,
                duration: None,
                label: "Cue".to_string(),
                category: "cue".to_string(),
                confidence: None,
                tags: Vec::new(),
                source_transform: String::new(),
                source_marker_ids: Vec::new(),
                metadata: JsonObject::new(),
            }],
            before_dependents: Vec::new(),
            after_dependents: Vec::new(),
        };

        let error = command.redo(&mut project).unwrap_err();

        assert!(error.to_string().contains("missing track"));
        assert!(project
            .markers
            .iter()
            .all(|marker| marker.id != "marker_orphan"));
    }

    #[test]
    fn history_discards_track_creation_undo_when_dependents_exist() {
        let mut project = project_with_source();
        let manual =
            create_manual_editable_track(&mut project, "track_source", "Manual Cues").unwrap();
        let mut history = EditHistory::new();
        history.push(TrackSnapshotCommand {
            track_id: manual.id.clone(),
            before: None,
            after: Some(manual.clone()),
            index: project
                .tracks
                .iter()
                .position(|track| track.id == manual.id)
                .unwrap(),
            before_markers: Vec::new(),
            after_markers: Vec::new(),
            before_job_runs: Vec::new(),
            after_job_runs: Vec::new(),
        });
        project.tracks.push(generated_track(
            "track_downstream",
            &manual.id,
            ResultState::Complete,
        ));

        assert!(!history.undo(&mut project).unwrap());

        assert!(project.tracks.iter().any(|track| track.id == manual.id));
        assert!(project
            .tracks
            .iter()
            .any(|track| track.id == "track_downstream"));
        assert!(!history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn history_push_after_undo_invalidates_redo_stack() {
        let mut project = project_with_editable_track();
        let marker = add_editable_marker(
            &mut project,
            "track_edit",
            EditableMarkerInput::cue(1.0, "Cue"),
        )
        .unwrap();
        let mut history = EditHistory::new();
        history.push(MarkerSnapshotCommand {
            track_id: "track_edit".to_string(),
            before: Vec::new(),
            after: vec![marker.clone()],
            before_dependents: Vec::new(),
            after_dependents: Vec::new(),
        });
        history.undo(&mut project).unwrap();

        history.push(MarkerSnapshotCommand {
            track_id: "track_edit".to_string(),
            before: Vec::new(),
            after: vec![marker],
            before_dependents: Vec::new(),
            after_dependents: Vec::new(),
        });

        assert!(!history.can_redo());
    }

    #[test]
    fn history_clear_resets_undo_redo_and_clean_state() {
        let mut history = EditHistory::new();
        history.push(ProjectSnapshotCommand {
            before: ProjectDocument::new("before", "Before"),
            after: ProjectDocument::new("after", "After"),
        });

        history.clear();

        assert!(!history.can_undo());
        assert!(!history.can_redo());
        assert!(history.is_clean());
    }

    #[test]
    fn history_project_snapshot_restores_project_fields() {
        let mut project = ProjectDocument::new("after", "After");
        project
            .ui_state
            .insert("timeline".to_string(), json!({"scroll_seconds": 4.0}));
        let mut before = ProjectDocument::new("before", "Before");
        before
            .ui_state
            .insert("timeline".to_string(), json!({"scroll_seconds": 1.0}));
        let command = ProjectSnapshotCommand {
            before,
            after: project.clone(),
        };

        command.undo(&mut project).unwrap();

        assert_eq!(project.id, "before");
        assert_eq!(project.name, "Before");
        assert_eq!(project.ui_state["timeline"]["scroll_seconds"], json!(1.0));
    }

    fn project_with_editable_track() -> ProjectDocument {
        let mut project = project_with_source();
        project.tracks.push(generated_track(
            "track_generated",
            "track_source",
            ResultState::Complete,
        ));
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

    fn marker_by_id<'a>(project: &'a ProjectDocument, marker_id: &str) -> &'a Marker {
        project
            .markers
            .iter()
            .find(|marker| marker.id == marker_id)
            .unwrap()
    }

    fn object(value: Value) -> JsonObject {
        value.as_object().cloned().unwrap()
    }
}
