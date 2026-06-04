use core::pin::Pin;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use autolight_core::cache::{track_dependency_hash, track_dependency_inputs};
use autolight_core::graph::{default_expanded_track_ids, find_track, source_track_id_for_context};
use autolight_core::history::{EditHistory, ProjectSnapshotCommand};
use autolight_core::markers::{
    add_editable_marker, bulk_update_editable_markers, create_manual_editable_track,
    delete_editable_marker, move_editable_markers, resize_editable_marker, update_editable_marker,
    BulkMarkerUpdate, EditableMarkerInput, MarkerUpdate,
};
use autolight_core::project::AudioAsset;
use autolight_core::project::{JsonObject, Marker, ProjectDocument, ResultState, Track, TrackType};
use autolight_core::transforms::{TransformRegistry, TransformSpec};
use autolight_jobs::queue::{
    JobRegistry, LocalJobQueue, ProducedMarker, TransformResult, TransformRunError,
};
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use serde_json::{json, Value};

use crate::timeline_model::{
    rust_demo_project, timeline_rows_json_with_state, RUST_DEMO_PROJECT_NAME,
};
use crate::transform_model::transform_specs_json;

const SMOKE_PROJECT_NAME: &str = "Autolight Rust Smoke";
const TIMELINE_DEFAULT_PIXELS_PER_SECOND: f64 = 96.0;
const TIMELINE_MIN_PIXELS_PER_SECOND: f64 = 24.0;
const TIMELINE_MAX_PIXELS_PER_SECOND: f64 = 240.0;
const TIMELINE_DEFAULT_VISIBLE_SECONDS: f64 = 8.0;
const TIMELINE_MIN_VISIBLE_SECONDS: f64 = 0.01;
const SNAP_THRESHOLD_PIXELS: f64 = 10.0;
const DEFAULT_MARKER_COLOR: &str = "cyan";
const MARKER_COLOR_OPTIONS: &[(&str, &str, &str)] = &[
    ("cyan", "Cyan", "#67e8f9"),
    ("green", "Green", "#a7f3d0"),
    ("amber", "Amber", "#fbbf24"),
    ("violet", "Violet", "#c4b5fd"),
    ("rose", "Rose", "#fda4af"),
    ("blue", "Blue", "#93c5fd"),
];

pub struct AppControllerState {
    project_name: QString,
    project_path: QString,
    last_error: QString,
    timeline_rows_json: QString,
    transform_specs_json: QString,
    selected_track_id: QString,
    timeline_duration_seconds: f64,
    timeline_pixels_per_second: f64,
    timeline_scroll_seconds: f64,
    timeline_visible_seconds: f64,
    is_dirty: bool,
    selected_track_can_rerun: bool,
    selected_track_has_running_job: bool,
    selected_track_is_editable: bool,
    selected_track_can_play: bool,
    selected_marker_ids_json: QString,
    selected_track_markers_json: QString,
    marker_color_options_json: QString,
    can_undo: bool,
    can_redo: bool,
    playback_source_path: QString,
    playback_position_seconds: f64,
    playback_duration_seconds: f64,
    playback_is_playing: bool,
    playback_last_error: QString,
    playback_volume: f64,
    project: ProjectDocument,
    transform_registry: TransformRegistry,
    job_queue: LocalJobQueue,
    next_track_number: u64,
    next_asset_number: u64,
    selected_marker_ids: Vec<String>,
    expanded_track_ids: BTreeSet<String>,
    edit_history: EditHistory,
    non_history_dirty: bool,
}

impl Default for AppControllerState {
    fn default() -> Self {
        let transform_registry = TransformRegistry::with_builtin_transforms();
        let transform_specs =
            transform_specs_json(&transform_registry).unwrap_or_else(|_| "[]".to_string());
        Self {
            project_name: QString::from(SMOKE_PROJECT_NAME),
            project_path: QString::default(),
            last_error: QString::default(),
            timeline_rows_json: QString::from("[]"),
            transform_specs_json: QString::from(&transform_specs),
            selected_track_id: QString::default(),
            timeline_duration_seconds: 0.0,
            timeline_pixels_per_second: TIMELINE_DEFAULT_PIXELS_PER_SECOND,
            timeline_scroll_seconds: 0.0,
            timeline_visible_seconds: TIMELINE_DEFAULT_VISIBLE_SECONDS,
            is_dirty: false,
            selected_track_can_rerun: false,
            selected_track_has_running_job: false,
            selected_track_is_editable: false,
            selected_track_can_play: false,
            selected_marker_ids_json: QString::from("[]"),
            selected_track_markers_json: QString::from("[]"),
            marker_color_options_json: QString::from(&marker_color_options_json()),
            can_undo: false,
            can_redo: false,
            playback_source_path: QString::default(),
            playback_position_seconds: 0.0,
            playback_duration_seconds: 0.0,
            playback_is_playing: false,
            playback_last_error: QString::default(),
            playback_volume: 1.0,
            project: ProjectDocument::new("project_empty", SMOKE_PROJECT_NAME),
            transform_registry,
            job_queue: LocalJobQueue::new(job_registry()),
            next_track_number: 1,
            next_asset_number: 1,
            selected_marker_ids: Vec::new(),
            expanded_track_ids: BTreeSet::new(),
            edit_history: EditHistory::new(),
            non_history_dirty: false,
        }
    }
}

impl AppControllerState {
    fn load_demo_project_state(&mut self) {
        self.project = rust_demo_project();
        self.project_name = QString::from(RUST_DEMO_PROJECT_NAME);
        self.project_path = QString::default();
        self.expanded_track_ids = default_expanded_track_ids(&self.project);
        self.selected_track_id = QString::from(
            self.project
                .tracks
                .first()
                .map(|track| track.id.as_str())
                .unwrap_or_default(),
        );
        self.selected_marker_ids.clear();
        self.unload_playback();
        self.reset_timeline_view_state();
        self.mark_clean();
        self.last_error = QString::default();
        self.refresh_view_state();
    }

    fn clear_project_state(&mut self) {
        self.project = ProjectDocument::new("project_empty", SMOKE_PROJECT_NAME);
        self.project_name = QString::from(SMOKE_PROJECT_NAME);
        self.project_path = QString::default();
        self.selected_track_id = QString::default();
        self.selected_marker_ids.clear();
        self.expanded_track_ids.clear();
        self.unload_playback();
        self.reset_timeline_view_state();
        self.mark_clean();
        self.last_error = QString::default();
        self.refresh_view_state();
    }

    fn select_track_state(&mut self, track_id: &str) {
        if find_track(&self.project, track_id).is_none() {
            self.set_error(format!("track not found: {track_id}"));
            return;
        }
        if self.selected_track_id.to_string() != track_id {
            self.selected_marker_ids.clear();
        }
        self.selected_track_id = QString::from(track_id);
        self.last_error = QString::default();
        self.refresh_view_state();
    }

    fn add_transform_track_state(
        &mut self,
        parent_track_id: &str,
        transform_id: &str,
        version: &str,
        params_json: &str,
    ) -> String {
        let params = match parse_params(params_json) {
            Ok(params) => params,
            Err(error) => {
                self.set_error(error);
                return String::new();
            }
        };
        let Some(parent) = find_track(&self.project, parent_track_id) else {
            self.set_error(format!("track not found: {parent_track_id}"));
            return String::new();
        };
        if parent.result_state != ResultState::Complete {
            self.set_error(format!("parent track is not complete: {}", parent.name));
            return String::new();
        }
        let spec = match self.transform_registry.get(transform_id, Some(version)) {
            Ok(spec) => spec.clone(),
            Err(error) => {
                self.set_error(error.to_string());
                return String::new();
            }
        };
        if !spec.is_compatible_parent(&self.project, parent_track_id) {
            self.set_error(parent_compatibility_error(parent, &spec));
            return String::new();
        }

        let dependency_hash = match dependency_hash_for_new_track(
            &self.project,
            parent_track_id,
            transform_id,
            version,
            &params,
        ) {
            Ok(hash) => hash,
            Err(error) => {
                self.set_error(error);
                return String::new();
            }
        };
        let track_id = self.next_track_id();
        self.project.tracks.push(Track {
            id: track_id.clone(),
            track_type: TrackType::Generated,
            name: spec.name,
            input_track_ids: vec![parent_track_id.to_string()],
            transform_id: transform_id.to_string(),
            transform_params: params,
            transform_version: version.to_string(),
            output_schema: spec.output_schema,
            dependency_hash,
            result_state: ResultState::Pending,
            cache_refs: Vec::new(),
            provenance: JsonObject::new(),
            error: String::new(),
        });
        self.expand_parent_for_new_child(parent_track_id);
        self.selected_track_id = QString::from(&track_id);
        self.selected_marker_ids.clear();
        self.mark_non_history_dirty();
        self.last_error = QString::default();
        self.refresh_view_state();
        track_id
    }

    fn run_track_state(&mut self, track_id: &str) -> String {
        match self.job_queue.submit(&mut self.project, track_id) {
            Ok(job_id) => {
                if let Err(error) = self.job_queue.run_next(&mut self.project) {
                    self.set_error(error.to_string());
                    return String::new();
                }
                self.mark_non_history_dirty();
                self.last_error = QString::default();
                self.refresh_view_state();
                job_id
            }
            Err(error) => {
                self.set_error(error.to_string());
                String::new()
            }
        }
    }

    fn rerun_track_state(&mut self, track_id: &str) -> String {
        self.run_track_state(track_id)
    }

    fn cancel_selected_job_state(&mut self) {
        let selected_track_id = self.selected_track_id.to_string();
        let Some(job_id) = latest_active_job_id(&self.project, &selected_track_id) else {
            return;
        };
        if let Err(error) = self.job_queue.cancel(&job_id) {
            self.set_error(error.to_string());
            return;
        }
        if let Err(error) = self.job_queue.run_next(&mut self.project) {
            self.set_error(error.to_string());
            return;
        }
        self.mark_non_history_dirty();
        self.refresh_view_state();
    }

    fn refresh_cache_status_state(&mut self) -> Vec<String> {
        let invalid_refs = self
            .job_queue
            .refresh_cache_validity(&mut self.project, |entry| {
                entry.validation_status == "valid"
            });
        if invalid_refs.is_empty() {
            self.last_error = QString::default();
        } else {
            self.set_error(format!("invalid cache artifacts: {}", invalid_refs.len()));
            self.mark_non_history_dirty();
        }
        self.refresh_view_state();
        invalid_refs
    }

    fn open_project_state(&mut self, path: &str) -> bool {
        let project_path = path_from_qml(path);
        let project = match ProjectDocument::load_path(&project_path) {
            Ok(project) => project,
            Err(error) => {
                self.set_error(error.to_string());
                return false;
            }
        };
        self.project = project;
        self.project_name = QString::from(&self.project.name);
        self.project_path = QString::from(project_path.to_string_lossy().to_string());
        self.expanded_track_ids = expanded_track_ids_from_project(&self.project)
            .unwrap_or_else(|| default_expanded_track_ids(&self.project));
        self.selected_track_id = QString::from(&selected_track_id_from_project(&self.project));
        self.restore_timeline_view_state();
        self.selected_marker_ids.clear();
        self.unload_playback();
        self.mark_clean();
        self.last_error = QString::default();
        self.refresh_view_state();
        true
    }

    fn save_project_state(&mut self, path: &str) -> bool {
        let project_path = if path.trim().is_empty() {
            let current = self.project_path.to_string();
            if current.is_empty() {
                self.set_error("project path is required");
                return false;
            }
            PathBuf::from(current)
        } else {
            path_from_qml(path)
        };
        let project_path = with_autolight_suffix(project_path);
        self.capture_timeline_ui_state();
        if let Err(error) = self.project.save_path(&project_path) {
            self.set_error(error.to_string());
            return false;
        }
        self.project_path = QString::from(project_path.to_string_lossy().to_string());
        self.mark_clean();
        self.last_error = QString::default();
        self.refresh_view_state();
        true
    }

    fn import_audio_state(&mut self, path: &str) -> String {
        let audio_path = path_from_qml(path);
        if !audio_path.is_file() {
            self.set_error(format!("No such file: {}", audio_path.display()));
            return String::new();
        }
        let metadata = match probe_wav_file(&audio_path) {
            Ok(metadata) => metadata,
            Err(error) => {
                self.set_error(error);
                return String::new();
            }
        };
        let asset_id = self.next_asset_id();
        let track_id = self.next_track_id();
        self.project.audio_assets.push(AudioAsset {
            id: asset_id.clone(),
            path: audio_path.to_string_lossy().to_string(),
            duration: metadata.duration,
            sample_rate: metadata.sample_rate,
            channels: metadata.channels,
            fingerprint: fingerprint_file(&audio_path).unwrap_or_default(),
            import_status: "online".to_string(),
            relink_hint: String::new(),
        });
        self.project.tracks.push(Track {
            id: track_id.clone(),
            track_type: TrackType::Source,
            name: audio_path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("Audio")
                .to_string(),
            input_track_ids: Vec::new(),
            transform_id: String::new(),
            transform_params: JsonObject::new(),
            transform_version: String::new(),
            output_schema: String::new(),
            dependency_hash: String::new(),
            result_state: ResultState::Complete,
            cache_refs: Vec::new(),
            provenance: json_object([("asset_id", json!(asset_id))]),
            error: String::new(),
        });
        self.selected_track_id = QString::from(&track_id);
        self.selected_marker_ids.clear();
        self.mark_non_history_dirty();
        self.last_error = QString::default();
        self.refresh_view_state();
        track_id
    }

    fn play_selected_track_state(&mut self) -> bool {
        let selected_track_id = self.selected_track_id.to_string();
        let Some(asset) = self
            .source_audio_asset_for_track_id(&selected_track_id)
            .cloned()
        else {
            self.set_error("selected track has no source audio");
            self.refresh_selected_state();
            return false;
        };
        if asset.import_status != "online" {
            self.set_error(format!("source audio is {}", asset.import_status));
            self.refresh_selected_state();
            return false;
        }
        if self.playback_source_path.to_string() != asset.path
            && !self.load_playback_source(&asset.path, asset.duration)
        {
            self.set_error(self.playback_last_error.to_string());
            self.refresh_selected_state();
            return false;
        }
        self.playback_is_playing = true;
        self.last_error = QString::default();
        self.refresh_selected_state();
        true
    }

    fn play_loaded_playback_state(&mut self) -> bool {
        if self.playback_source_path.to_string().is_empty() {
            self.playback_last_error = QString::from("no audio source loaded");
            self.refresh_selected_state();
            return false;
        }
        self.playback_is_playing = true;
        self.playback_last_error = QString::default();
        self.refresh_selected_state();
        true
    }

    fn pause_playback_state(&mut self) {
        self.playback_is_playing = false;
        self.refresh_selected_state();
    }

    fn stop_playback_state(&mut self) {
        self.playback_is_playing = false;
        self.playback_position_seconds = 0.0;
        self.refresh_selected_state();
    }

    fn seek_playback_state(&mut self, seconds: f64) {
        self.playback_position_seconds =
            finite_non_negative(seconds).min(self.playback_duration_seconds.max(0.0));
        self.refresh_selected_state();
    }

    fn nudge_playback_state(&mut self, delta_seconds: f64) {
        self.seek_playback_state(self.playback_position_seconds + delta_seconds);
    }

    fn set_playback_volume_state(&mut self, value: f64) {
        self.playback_volume = finite_non_negative(value).clamp(0.0, 1.0);
        self.refresh_selected_state();
    }

    fn set_timeline_zoom_state(&mut self, pixels_per_second: f64) {
        if !pixels_per_second.is_finite() {
            return;
        }
        self.timeline_pixels_per_second = pixels_per_second.clamp(
            TIMELINE_MIN_PIXELS_PER_SECOND,
            TIMELINE_MAX_PIXELS_PER_SECOND,
        );
        self.clamp_timeline_scroll();
        self.refresh_selected_state();
    }

    fn set_timeline_scroll_seconds_state(&mut self, seconds: f64) {
        if !seconds.is_finite() {
            return;
        }
        self.timeline_scroll_seconds = finite_non_negative(seconds);
        self.clamp_timeline_scroll();
        self.refresh_selected_state();
    }

    fn set_timeline_visible_seconds_state(&mut self, seconds: f64) {
        if !seconds.is_finite() {
            return;
        }
        self.timeline_visible_seconds =
            finite_non_negative(seconds).max(TIMELINE_MIN_VISIBLE_SECONDS);
        self.clamp_timeline_scroll();
        self.refresh_selected_state();
    }

    fn snap_timeline_time_state(&self, seconds: f64, bypass_snap: bool) -> f64 {
        if bypass_snap || !seconds.is_finite() {
            return seconds;
        }
        let threshold_seconds = SNAP_THRESHOLD_PIXELS / self.timeline_pixels_per_second.max(1.0);
        let visible_track_ids = self.visible_track_ids();
        self.project
            .markers
            .iter()
            .filter(|marker| visible_track_ids.contains(&marker.track_id))
            .filter_map(|marker| {
                let distance = (marker.timestamp - seconds).abs();
                (distance <= threshold_seconds).then_some((distance, marker.timestamp))
            })
            .min_by(|left, right| left.0.total_cmp(&right.0))
            .map(|(_, timestamp)| timestamp)
            .unwrap_or(seconds)
    }

    fn refresh_view_state(&mut self) {
        let selected_marker_ids = self.selected_marker_ids_set();
        match timeline_rows_json_with_state(
            &self.project,
            &self.expanded_track_ids,
            &selected_marker_ids,
        ) {
            Ok(rows_json) => {
                self.timeline_rows_json = QString::from(&rows_json);
            }
            Err(error) => {
                self.set_error(error.to_string());
            }
        }
        self.transform_specs_json = QString::from(
            &transform_specs_json(&self.transform_registry).unwrap_or_else(|_| "[]".to_string()),
        );
        self.timeline_duration_seconds = self
            .project
            .audio_assets
            .iter()
            .map(|asset| asset.duration)
            .fold(self.playback_duration_seconds, f64::max);
        self.clamp_timeline_scroll();
        self.refresh_selected_state();
    }

    fn refresh_selected_state(&mut self) {
        self.reconcile_selection_with_project();
        let selected_track_id = self.selected_track_id.to_string();
        let selected_track = find_track(&self.project, &selected_track_id);
        self.selected_track_can_rerun = selected_track.is_some_and(|track| {
            track.track_type == TrackType::Generated && track.result_state != ResultState::Running
        });
        self.selected_track_has_running_job =
            latest_active_job_id(&self.project, &selected_track_id).is_some();
        self.selected_track_is_editable =
            selected_track.is_some_and(|track| track.track_type == TrackType::Editable);
        self.selected_track_can_play = self
            .source_audio_asset_for_track_id(&selected_track_id)
            .is_some_and(|asset| asset.import_status == "online");
        self.selected_marker_ids_json = QString::from(&json_string(&self.selected_marker_ids));
        self.selected_track_markers_json =
            QString::from(&json_string(&self.selected_track_marker_payloads()));
        self.marker_color_options_json = QString::from(&marker_color_options_json());
        self.can_undo = self.edit_history.can_undo();
        self.can_redo = self.edit_history.can_redo();
        self.sync_dirty_from_history();
    }

    fn add_manual_cue_track_state(&mut self, name: &str) -> String {
        let selected_track_id = self.selected_track_id.to_string();
        let before = self.project.clone();
        let track = match create_manual_editable_track(
            &mut self.project,
            &selected_track_id,
            if name.is_empty() { "Manual Cues" } else { name },
        ) {
            Ok(track) => track,
            Err(error) => {
                self.set_error(error.to_string());
                return String::new();
            }
        };
        if let Some(parent_track_id) = track.input_track_ids.first() {
            self.expand_parent_for_new_child(parent_track_id);
        }
        self.selected_track_id = QString::from(&track.id);
        self.selected_marker_ids.clear();
        self.record_project_snapshot(before);
        self.last_error = QString::default();
        self.refresh_view_state();
        track.id
    }

    fn create_editable_track_from_track_state(&mut self, source_track_id: &str) -> String {
        if find_track(&self.project, source_track_id).is_none() {
            self.set_error(format!("track not found: {source_track_id}"));
            return String::new();
        }
        let mut source_markers = self
            .project
            .markers
            .iter()
            .filter(|marker| marker.track_id == source_track_id)
            .cloned()
            .collect::<Vec<_>>();
        if source_markers.is_empty() {
            self.set_error("source track has no markers");
            return String::new();
        }
        source_markers.sort_by(|left, right| {
            left.timestamp
                .total_cmp(&right.timestamp)
                .then_with(|| left.id.cmp(&right.id))
        });

        let before = self.project.clone();
        let track_id = self.next_track_id();
        let source_marker_ids = source_markers
            .iter()
            .map(|marker| marker.id.clone())
            .collect::<Vec<_>>();
        let track = Track {
            id: track_id.clone(),
            track_type: TrackType::Editable,
            name: "Editable Cues".to_string(),
            input_track_ids: vec![source_track_id.to_string()],
            transform_id: String::new(),
            transform_params: JsonObject::new(),
            transform_version: String::new(),
            output_schema: String::new(),
            dependency_hash: String::new(),
            result_state: ResultState::Complete,
            cache_refs: Vec::new(),
            provenance: json_object([
                ("source_track_id", json!(source_track_id)),
                ("source_marker_ids", json!(source_marker_ids)),
            ]),
            error: String::new(),
        };
        self.project.tracks.push(track);
        for (index, source_marker) in source_markers.iter().enumerate() {
            self.project.markers.push(Marker {
                id: self.next_marker_id(&track_id, index + 1),
                track_id: track_id.clone(),
                timestamp: source_marker.timestamp,
                duration: source_marker.duration,
                label: source_marker.label.clone(),
                category: source_marker.category.clone(),
                confidence: source_marker.confidence,
                tags: source_marker.tags.clone(),
                source_transform: source_marker.source_transform.clone(),
                source_marker_ids: vec![source_marker.id.clone()],
                metadata: source_marker.metadata.clone(),
            });
        }
        self.expand_parent_for_new_child(source_track_id);
        self.selected_track_id = QString::from(&track_id);
        self.selected_marker_ids.clear();
        self.record_project_snapshot(before);
        self.last_error = QString::default();
        self.refresh_view_state();
        track_id
    }

    fn set_track_expanded_state(&mut self, track_id: &str, expanded: bool) -> bool {
        if find_track(&self.project, track_id).is_none() {
            self.set_error(format!("track not found: {track_id}"));
            return false;
        }
        let has_children = self.project.tracks.iter().any(|track| {
            track
                .input_track_ids
                .first()
                .is_some_and(|id| id == track_id)
        });
        if !has_children {
            return false;
        }
        let changed = if expanded {
            self.expanded_track_ids.insert(track_id.to_string())
        } else {
            self.expanded_track_ids.remove(track_id)
        };
        if changed {
            self.project.ui_state.insert(
                "expanded_track_ids".to_string(),
                json!(self.expanded_track_ids.iter().cloned().collect::<Vec<_>>()),
            );
            if !expanded
                && !self
                    .visible_track_ids()
                    .contains(&self.selected_track_id.to_string())
            {
                self.selected_track_id = QString::from(track_id);
                self.selected_marker_ids.clear();
            }
            self.mark_non_history_dirty();
            self.last_error = QString::default();
            self.refresh_view_state();
        }
        changed
    }

    fn add_marker_to_selected_track_with_duration_state(
        &mut self,
        timestamp: f64,
        duration: f64,
        label: &str,
        category: &str,
        color: &str,
    ) -> String {
        let track_id = self.selected_track_id.to_string();
        let before = self.project.clone();
        let marker = match add_editable_marker(
            &mut self.project,
            &track_id,
            EditableMarkerInput {
                timestamp,
                duration: Some(duration),
                label: label.to_string(),
                category: category.to_string(),
                color: color.to_string(),
            },
        ) {
            Ok(marker) => marker,
            Err(error) => {
                self.set_error(error.to_string());
                return String::new();
            }
        };
        self.selected_marker_ids = vec![marker.id.clone()];
        self.record_project_snapshot(before);
        self.last_error = QString::default();
        self.refresh_view_state();
        marker.id
    }

    fn delete_marker_from_selected_track_state(&mut self, marker_id: &str) -> bool {
        self.delete_markers_from_selected_track(&[marker_id.to_string()]) > 0
    }

    fn delete_selected_markers_state(&mut self) -> i32 {
        if self.selected_marker_ids.is_empty() {
            self.set_error("select at least one marker to delete");
            return 0;
        }
        let marker_ids = self.selected_marker_ids.clone();
        self.delete_markers_from_selected_track(&marker_ids) as i32
    }

    fn update_selected_marker_with_duration_state(
        &mut self,
        timestamp: f64,
        duration: f64,
        label: &str,
        category: &str,
        color: &str,
    ) -> bool {
        if self.selected_marker_ids.len() != 1 {
            self.set_error("select one marker to update");
            return false;
        }
        let track_id = self.selected_track_id.to_string();
        let marker_id = self.selected_marker_ids[0].clone();
        let before = self.project.clone();
        if let Err(error) = update_editable_marker(
            &mut self.project,
            &track_id,
            &marker_id,
            MarkerUpdate {
                timestamp,
                duration: Some(duration),
                label: label.to_string(),
                category: category.to_string(),
                color: color.to_string(),
            },
        ) {
            self.set_error(error.to_string());
            return false;
        }
        self.record_project_snapshot(before);
        self.last_error = QString::default();
        self.refresh_view_state();
        true
    }

    fn bulk_update_selected_markers_state(
        &mut self,
        label: &str,
        category: &str,
        color: &str,
    ) -> i32 {
        let track_id = self.selected_track_id.to_string();
        let before = self.project.clone();
        let updated = match bulk_update_editable_markers(
            &mut self.project,
            &track_id,
            &self.selected_marker_ids,
            BulkMarkerUpdate {
                label: label.to_string(),
                category: category.to_string(),
                color: color.to_string(),
            },
        ) {
            Ok(updated) => updated,
            Err(error) => {
                self.set_error(error.to_string());
                return 0;
            }
        };
        self.record_project_snapshot(before);
        self.last_error = QString::default();
        self.refresh_view_state();
        updated as i32
    }

    fn toggle_marker_selection_state(&mut self, marker_id: &str, additive: bool) {
        let track_id = self.selected_track_id.to_string();
        let marker_exists = self
            .project
            .markers
            .iter()
            .any(|marker| marker.track_id == track_id && marker.id == marker_id);
        if !marker_exists {
            self.set_error(format!("marker not found: {marker_id}"));
            return;
        }
        if additive {
            if let Some(index) = self
                .selected_marker_ids
                .iter()
                .position(|selected_id| selected_id == marker_id)
            {
                self.selected_marker_ids.remove(index);
            } else {
                self.selected_marker_ids.push(marker_id.to_string());
            }
        } else {
            self.selected_marker_ids = vec![marker_id.to_string()];
        }
        self.last_error = QString::default();
        self.refresh_view_state();
    }

    fn move_selected_markers_state(&mut self, delta_seconds: f64, bypass_snap: bool) -> bool {
        if self.selected_marker_ids.is_empty() {
            self.set_error("select at least one marker to move");
            return false;
        }
        let track_id = self.selected_track_id.to_string();
        let delta_seconds = if !bypass_snap && self.selected_marker_ids.len() == 1 {
            self.project
                .markers
                .iter()
                .find(|marker| {
                    marker.track_id == track_id && marker.id == self.selected_marker_ids[0]
                })
                .map(|marker| {
                    self.snap_timeline_time_state(marker.timestamp + delta_seconds, false)
                        - marker.timestamp
                })
                .unwrap_or(delta_seconds)
        } else {
            delta_seconds
        };
        let before = self.project.clone();
        if let Err(error) = move_editable_markers(
            &mut self.project,
            &track_id,
            &self.selected_marker_ids,
            delta_seconds,
        ) {
            self.set_error(error.to_string());
            return false;
        }
        self.record_project_snapshot(before);
        self.last_error = QString::default();
        self.refresh_view_state();
        true
    }

    fn resize_marker_state(&mut self, marker_id: &str, duration: f64) -> bool {
        let track_id = self.selected_track_id.to_string();
        let before = self.project.clone();
        if let Err(error) =
            resize_editable_marker(&mut self.project, &track_id, marker_id, duration)
        {
            self.set_error(error.to_string());
            return false;
        }
        self.record_project_snapshot(before);
        self.last_error = QString::default();
        self.refresh_view_state();
        true
    }

    fn undo_state(&mut self) -> bool {
        match self.edit_history.undo(&mut self.project) {
            Ok(changed) => {
                if changed {
                    self.last_error = QString::default();
                    self.refresh_view_state();
                } else {
                    self.refresh_selected_state();
                }
                changed
            }
            Err(error) => {
                self.set_error(error.to_string());
                false
            }
        }
    }

    fn redo_state(&mut self) -> bool {
        match self.edit_history.redo(&mut self.project) {
            Ok(changed) => {
                if changed {
                    self.last_error = QString::default();
                    self.refresh_view_state();
                } else {
                    self.refresh_selected_state();
                }
                changed
            }
            Err(error) => {
                self.set_error(error.to_string());
                false
            }
        }
    }

    fn delete_markers_from_selected_track(&mut self, marker_ids: &[String]) -> usize {
        let track_id = self.selected_track_id.to_string();
        let before = self.project.clone();
        let mut deleted_ids = Vec::new();
        for marker_id in marker_ids {
            match delete_editable_marker(&mut self.project, &track_id, marker_id) {
                Ok(true) => deleted_ids.push(marker_id.clone()),
                Ok(false) => {}
                Err(error) => {
                    self.set_error(error.to_string());
                    return 0;
                }
            }
        }
        if !deleted_ids.is_empty() {
            self.record_project_snapshot(before);
        }
        let ids_to_clear = if deleted_ids.is_empty() {
            marker_ids.iter().collect::<BTreeSet<_>>()
        } else {
            deleted_ids.iter().collect::<BTreeSet<_>>()
        };
        self.selected_marker_ids
            .retain(|marker_id| !ids_to_clear.contains(marker_id));
        self.last_error = QString::default();
        self.refresh_view_state();
        deleted_ids.len()
    }

    fn set_error(&mut self, error: impl Into<String>) {
        self.last_error = QString::from(&error.into());
    }

    fn next_track_id(&mut self) -> String {
        loop {
            let candidate = format!("track_rust_{:04}", self.next_track_number);
            self.next_track_number += 1;
            if find_track(&self.project, &candidate).is_none() {
                return candidate;
            }
        }
    }

    fn next_asset_id(&mut self) -> String {
        loop {
            let candidate = format!("asset_rust_{:04}", self.next_asset_number);
            self.next_asset_number += 1;
            if !self
                .project
                .audio_assets
                .iter()
                .any(|asset| asset.id == candidate)
            {
                return candidate;
            }
        }
    }

    fn next_marker_id(&self, track_id: &str, ordinal: usize) -> String {
        let mut counter = ordinal;
        loop {
            let candidate = format!("marker_{track_id}_{counter:04}");
            if !self
                .project
                .markers
                .iter()
                .any(|marker| marker.id == candidate)
            {
                return candidate;
            }
            counter += 1;
        }
    }

    fn record_project_snapshot(&mut self, before: ProjectDocument) {
        if before == self.project {
            self.sync_dirty_from_history();
            return;
        }
        self.edit_history.push(ProjectSnapshotCommand {
            before,
            after: self.project.clone(),
        });
        self.sync_dirty_from_history();
    }

    fn mark_clean(&mut self) {
        self.edit_history.clear();
        self.edit_history.mark_clean();
        self.non_history_dirty = false;
        self.sync_dirty_from_history();
    }

    fn mark_non_history_dirty(&mut self) {
        self.non_history_dirty = true;
        self.sync_dirty_from_history();
    }

    fn sync_dirty_from_history(&mut self) {
        self.is_dirty = self.non_history_dirty || !self.edit_history.is_clean();
        self.can_undo = self.edit_history.can_undo();
        self.can_redo = self.edit_history.can_redo();
    }

    fn selected_marker_ids_set(&self) -> BTreeSet<String> {
        self.selected_marker_ids.iter().cloned().collect()
    }

    fn selected_track_marker_payloads(&self) -> Vec<Value> {
        let selected_track_id = self.selected_track_id.to_string();
        let selected_marker_ids = self.selected_marker_ids_set();
        let mut markers = self
            .project
            .markers
            .iter()
            .filter(|marker| marker.track_id == selected_track_id)
            .collect::<Vec<_>>();
        markers.sort_by(|left, right| {
            left.timestamp
                .total_cmp(&right.timestamp)
                .then_with(|| left.id.cmp(&right.id))
        });
        markers
            .into_iter()
            .map(|marker| {
                let color_key = marker_color_key(marker);
                json!({
                    "id": marker.id.clone(),
                    "timestamp": marker.timestamp,
                    "duration": marker.duration.unwrap_or(0.0),
                    "label": marker.label.clone(),
                    "category": marker.category.clone(),
                    "color": marker_display_color_for_key(color_key),
                    "colorKey": color_key,
                    "selected": selected_marker_ids.contains(&marker.id),
                })
            })
            .collect()
    }

    fn source_audio_asset_for_track_id(&self, track_id: &str) -> Option<&AudioAsset> {
        let source_track_id = source_track_id_for_context(&self.project, track_id)?;
        let source_track = find_track(&self.project, &source_track_id)?;
        let asset_id = source_track
            .provenance
            .get("asset_id")
            .and_then(Value::as_str)?;
        self.project
            .audio_assets
            .iter()
            .find(|asset| asset.id == asset_id)
    }

    fn load_playback_source(&mut self, path: &str, duration_seconds: f64) -> bool {
        if !Path::new(path).is_file() {
            self.unload_playback();
            self.playback_last_error = QString::from(&format!("audio file not found: {path}"));
            return false;
        }
        self.playback_source_path = QString::from(path);
        self.playback_duration_seconds = finite_non_negative(duration_seconds);
        self.playback_position_seconds = 0.0;
        self.playback_is_playing = false;
        self.playback_last_error = QString::default();
        true
    }

    fn unload_playback(&mut self) {
        self.playback_source_path = QString::default();
        self.playback_duration_seconds = 0.0;
        self.playback_position_seconds = 0.0;
        self.playback_is_playing = false;
        self.playback_last_error = QString::default();
    }

    fn capture_timeline_ui_state(&mut self) {
        self.project.ui_state.insert(
            "expanded_track_ids".to_string(),
            json!(self.expanded_track_ids.iter().cloned().collect::<Vec<_>>()),
        );
        self.project.ui_state.insert(
            "timeline".to_string(),
            json!({
                "selected_track_id": self.selected_track_id.to_string(),
                "pixels_per_second": self.timeline_pixels_per_second,
                "scroll_seconds": self.timeline_scroll_seconds,
            }),
        );
    }

    fn restore_timeline_view_state(&mut self) {
        self.timeline_pixels_per_second = self
            .project
            .ui_state
            .get("timeline")
            .and_then(|timeline| timeline.get("pixels_per_second"))
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
            .unwrap_or(TIMELINE_DEFAULT_PIXELS_PER_SECOND)
            .clamp(
                TIMELINE_MIN_PIXELS_PER_SECOND,
                TIMELINE_MAX_PIXELS_PER_SECOND,
            );
        self.timeline_scroll_seconds = self
            .project
            .ui_state
            .get("timeline")
            .and_then(|timeline| timeline.get("scroll_seconds"))
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
            .map(finite_non_negative)
            .unwrap_or(0.0);
        self.timeline_visible_seconds = TIMELINE_DEFAULT_VISIBLE_SECONDS;
    }

    fn reset_timeline_view_state(&mut self) {
        self.timeline_pixels_per_second = TIMELINE_DEFAULT_PIXELS_PER_SECOND;
        self.timeline_scroll_seconds = 0.0;
        self.timeline_visible_seconds = TIMELINE_DEFAULT_VISIBLE_SECONDS;
    }

    fn clamp_timeline_scroll(&mut self) {
        let max_scroll = (self.timeline_duration_seconds - self.timeline_visible_seconds).max(0.0);
        self.timeline_scroll_seconds = self.timeline_scroll_seconds.clamp(0.0, max_scroll);
    }

    fn reconcile_selection_with_project(&mut self) {
        let selected_track_id = self.selected_track_id.to_string();
        if selected_track_id.is_empty() {
            self.selected_marker_ids.clear();
            return;
        }
        if find_track(&self.project, &selected_track_id).is_none() {
            self.selected_track_id = QString::default();
            self.selected_marker_ids.clear();
            return;
        }
        let valid_marker_ids = self
            .project
            .markers
            .iter()
            .filter(|marker| marker.track_id == selected_track_id)
            .map(|marker| marker.id.as_str())
            .collect::<BTreeSet<_>>();
        self.selected_marker_ids
            .retain(|marker_id| valid_marker_ids.contains(marker_id.as_str()));
    }

    fn expand_parent_for_new_child(&mut self, parent_track_id: &str) {
        if !parent_track_id.is_empty() {
            self.expanded_track_ids.insert(parent_track_id.to_string());
        }
    }

    fn visible_track_ids(&self) -> BTreeSet<String> {
        self.timeline_rows()
            .into_iter()
            .map(|row| row.track_id)
            .collect()
    }

    fn timeline_rows(&self) -> Vec<crate::timeline_model::TimelineRow> {
        let selected_marker_ids = self.selected_marker_ids_set();
        crate::timeline_model::timeline_rows_for_project_with_state(
            &self.project,
            &self.expanded_track_ids,
            &selected_marker_ids,
        )
    }

    fn qproperty_values(&self) -> ControllerPropertyValues {
        ControllerPropertyValues {
            project_name: self.project_name.clone(),
            project_path: self.project_path.clone(),
            last_error: self.last_error.clone(),
            timeline_rows_json: self.timeline_rows_json.clone(),
            transform_specs_json: self.transform_specs_json.clone(),
            selected_track_id: self.selected_track_id.clone(),
            timeline_duration_seconds: self.timeline_duration_seconds,
            timeline_pixels_per_second: self.timeline_pixels_per_second,
            timeline_scroll_seconds: self.timeline_scroll_seconds,
            timeline_visible_seconds: self.timeline_visible_seconds,
            is_dirty: self.is_dirty,
            selected_track_can_rerun: self.selected_track_can_rerun,
            selected_track_has_running_job: self.selected_track_has_running_job,
            selected_track_is_editable: self.selected_track_is_editable,
            selected_track_can_play: self.selected_track_can_play,
            selected_marker_ids_json: self.selected_marker_ids_json.clone(),
            selected_track_markers_json: self.selected_track_markers_json.clone(),
            marker_color_options_json: self.marker_color_options_json.clone(),
            can_undo: self.can_undo,
            can_redo: self.can_redo,
            playback_source_path: self.playback_source_path.clone(),
            playback_position_seconds: self.playback_position_seconds,
            playback_duration_seconds: self.playback_duration_seconds,
            playback_is_playing: self.playback_is_playing,
            playback_last_error: self.playback_last_error.clone(),
            playback_volume: self.playback_volume,
        }
    }
}

struct ControllerPropertyValues {
    project_name: QString,
    project_path: QString,
    last_error: QString,
    timeline_rows_json: QString,
    transform_specs_json: QString,
    selected_track_id: QString,
    timeline_duration_seconds: f64,
    timeline_pixels_per_second: f64,
    timeline_scroll_seconds: f64,
    timeline_visible_seconds: f64,
    is_dirty: bool,
    selected_track_can_rerun: bool,
    selected_track_has_running_job: bool,
    selected_track_is_editable: bool,
    selected_track_can_play: bool,
    selected_marker_ids_json: QString,
    selected_track_markers_json: QString,
    marker_color_options_json: QString,
    can_undo: bool,
    can_redo: bool,
    playback_source_path: QString,
    playback_position_seconds: f64,
    playback_duration_seconds: f64,
    playback_is_playing: bool,
    playback_last_error: QString,
    playback_volume: f64,
}

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");

        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, project_name, cxx_name = "projectName")]
        #[qproperty(QString, project_path, cxx_name = "projectPath")]
        #[qproperty(QString, last_error, cxx_name = "lastError")]
        #[qproperty(QString, timeline_rows_json, cxx_name = "timelineRowsJson")]
        #[qproperty(QString, transform_specs_json, cxx_name = "transformSpecsJson")]
        #[qproperty(QString, selected_track_id, cxx_name = "selectedTrackId")]
        #[qproperty(f64, timeline_duration_seconds, cxx_name = "timelineDurationSeconds")]
        #[qproperty(f64, timeline_pixels_per_second, cxx_name = "timelinePixelsPerSecond")]
        #[qproperty(f64, timeline_scroll_seconds, cxx_name = "timelineScrollSeconds")]
        #[qproperty(f64, timeline_visible_seconds, cxx_name = "timelineVisibleSeconds")]
        #[qproperty(bool, is_dirty, cxx_name = "isDirty")]
        #[qproperty(bool, selected_track_can_rerun, cxx_name = "selectedTrackCanRerun")]
        #[qproperty(
            bool,
            selected_track_has_running_job,
            cxx_name = "selectedTrackHasRunningJob"
        )]
        #[qproperty(bool, selected_track_is_editable, cxx_name = "selectedTrackIsEditable")]
        #[qproperty(bool, selected_track_can_play, cxx_name = "selectedTrackCanPlay")]
        #[qproperty(QString, selected_marker_ids_json, cxx_name = "selectedMarkerIdsJson")]
        #[qproperty(
            QString,
            selected_track_markers_json,
            cxx_name = "selectedTrackMarkersJson"
        )]
        #[qproperty(
            QString,
            marker_color_options_json,
            cxx_name = "markerColorOptionsJson"
        )]
        #[qproperty(bool, can_undo, cxx_name = "canUndo")]
        #[qproperty(bool, can_redo, cxx_name = "canRedo")]
        #[qproperty(QString, playback_source_path, cxx_name = "playbackSourcePath")]
        #[qproperty(f64, playback_position_seconds, cxx_name = "playbackPositionSeconds")]
        #[qproperty(f64, playback_duration_seconds, cxx_name = "playbackDurationSeconds")]
        #[qproperty(bool, playback_is_playing, cxx_name = "playbackIsPlaying")]
        #[qproperty(QString, playback_last_error, cxx_name = "playbackLastError")]
        #[qproperty(f64, playback_volume, cxx_name = "playbackVolume")]
        type AppController = super::AppControllerState;

        #[qinvokable]
        #[cxx_name = "newProject"]
        fn new_project(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "loadDemoProject"]
        fn load_demo_project(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "openProject"]
        fn open_project(self: Pin<&mut Self>, path: QString) -> bool;

        #[qinvokable]
        #[cxx_name = "saveProject"]
        fn save_project(self: Pin<&mut Self>, path: QString) -> bool;

        #[qinvokable]
        #[cxx_name = "importAudio"]
        fn import_audio(self: Pin<&mut Self>, path: QString) -> QString;

        #[qinvokable]
        #[cxx_name = "selectTrack"]
        fn select_track(self: Pin<&mut Self>, track_id: QString);

        #[qinvokable]
        #[cxx_name = "addTransformTrack"]
        fn add_transform_track(
            self: Pin<&mut Self>,
            parent_track_id: QString,
            transform_id: QString,
            version: QString,
            params_json: QString,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "runTrack"]
        fn run_track(self: Pin<&mut Self>, track_id: QString) -> QString;

        #[qinvokable]
        #[cxx_name = "rerunTrack"]
        fn rerun_track(self: Pin<&mut Self>, track_id: QString) -> QString;

        #[qinvokable]
        #[cxx_name = "cancelSelectedJob"]
        fn cancel_selected_job(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "refreshCacheStatus"]
        fn refresh_cache_status(self: Pin<&mut Self>) -> QString;

        #[qinvokable]
        #[cxx_name = "addManualCueTrack"]
        fn add_manual_cue_track(self: Pin<&mut Self>, name: QString) -> QString;

        #[qinvokable]
        #[cxx_name = "createEditableTrackFromTrack"]
        fn create_editable_track_from_track(
            self: Pin<&mut Self>,
            source_track_id: QString,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "setTrackExpanded"]
        fn set_track_expanded(self: Pin<&mut Self>, track_id: QString, expanded: bool) -> bool;

        #[qinvokable]
        #[cxx_name = "addMarkerToSelectedTrackWithDuration"]
        fn add_marker_to_selected_track_with_duration(
            self: Pin<&mut Self>,
            timestamp: f64,
            duration: f64,
            label: QString,
            category: QString,
            color: QString,
        ) -> QString;

        #[qinvokable]
        #[cxx_name = "deleteMarkerFromSelectedTrack"]
        fn delete_marker_from_selected_track(self: Pin<&mut Self>, marker_id: QString) -> bool;

        #[qinvokable]
        #[cxx_name = "deleteSelectedMarkers"]
        fn delete_selected_markers(self: Pin<&mut Self>) -> i32;

        #[qinvokable]
        #[cxx_name = "updateSelectedMarkerWithDuration"]
        fn update_selected_marker_with_duration(
            self: Pin<&mut Self>,
            timestamp: f64,
            duration: f64,
            label: QString,
            category: QString,
            color: QString,
        ) -> bool;

        #[qinvokable]
        #[cxx_name = "bulkUpdateSelectedMarkers"]
        fn bulk_update_selected_markers(
            self: Pin<&mut Self>,
            label: QString,
            category: QString,
            color: QString,
        ) -> i32;

        #[qinvokable]
        #[cxx_name = "toggleMarkerSelection"]
        fn toggle_marker_selection(self: Pin<&mut Self>, marker_id: QString, additive: bool);

        #[qinvokable]
        #[cxx_name = "moveSelectedMarkers"]
        fn move_selected_markers(
            self: Pin<&mut Self>,
            delta_seconds: f64,
            bypass_snap: bool,
        ) -> bool;

        #[qinvokable]
        #[cxx_name = "resizeMarker"]
        fn resize_marker(self: Pin<&mut Self>, marker_id: QString, duration: f64) -> bool;

        #[qinvokable]
        #[cxx_name = "undo"]
        fn undo(self: Pin<&mut Self>) -> bool;

        #[qinvokable]
        #[cxx_name = "redo"]
        fn redo(self: Pin<&mut Self>) -> bool;

        #[qinvokable]
        #[cxx_name = "playSelectedTrack"]
        fn play_selected_track(self: Pin<&mut Self>) -> bool;

        #[qinvokable]
        #[cxx_name = "playLoadedPlayback"]
        fn play_loaded_playback(self: Pin<&mut Self>) -> bool;

        #[qinvokable]
        #[cxx_name = "pausePlayback"]
        fn pause_playback(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "stopPlayback"]
        fn stop_playback(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "seekPlayback"]
        fn seek_playback(self: Pin<&mut Self>, seconds: f64);

        #[qinvokable]
        #[cxx_name = "nudgePlayback"]
        fn nudge_playback(self: Pin<&mut Self>, delta_seconds: f64);

        #[qinvokable]
        #[cxx_name = "setPlaybackVolumeValue"]
        fn set_playback_volume_invokable(self: Pin<&mut Self>, value: f64);

        #[qinvokable]
        #[cxx_name = "setTimelineZoom"]
        fn set_timeline_zoom(self: Pin<&mut Self>, pixels_per_second: f64);

        #[qinvokable]
        #[cxx_name = "applyTimelineScrollSeconds"]
        fn set_timeline_scroll_seconds_invokable(self: Pin<&mut Self>, seconds: f64);

        #[qinvokable]
        #[cxx_name = "applyTimelineVisibleSeconds"]
        fn set_timeline_visible_seconds_invokable(self: Pin<&mut Self>, seconds: f64);

        #[qinvokable]
        #[cxx_name = "snapTimelineTime"]
        fn snap_timeline_time(self: Pin<&mut Self>, seconds: f64, bypass_snap: bool) -> f64;
    }
}

impl qobject::AppController {
    pub fn new_project(mut self: Pin<&mut Self>) {
        let values = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            state.clear_project_state();
            state.qproperty_values()
        };
        self.apply_values(values);
    }

    pub fn load_demo_project(mut self: Pin<&mut Self>) {
        let values = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            state.load_demo_project_state();
            state.qproperty_values()
        };
        self.apply_values(values);
    }

    pub fn open_project(mut self: Pin<&mut Self>, path: QString) -> bool {
        let (values, opened) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let opened = state.open_project_state(&path.to_string());
            (state.qproperty_values(), opened)
        };
        self.apply_values(values);
        opened
    }

    pub fn save_project(mut self: Pin<&mut Self>, path: QString) -> bool {
        let (values, saved) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let saved = state.save_project_state(&path.to_string());
            (state.qproperty_values(), saved)
        };
        self.apply_values(values);
        saved
    }

    pub fn import_audio(mut self: Pin<&mut Self>, path: QString) -> QString {
        let (values, track_id) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let track_id = state.import_audio_state(&path.to_string());
            (state.qproperty_values(), track_id)
        };
        self.apply_values(values);
        QString::from(&track_id)
    }

    pub fn select_track(mut self: Pin<&mut Self>, track_id: QString) {
        let values = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            state.select_track_state(&track_id.to_string());
            state.qproperty_values()
        };
        self.apply_values(values);
    }

    pub fn add_transform_track(
        mut self: Pin<&mut Self>,
        parent_track_id: QString,
        transform_id: QString,
        version: QString,
        params_json: QString,
    ) -> QString {
        let (values, track_id) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let track_id = state.add_transform_track_state(
                &parent_track_id.to_string(),
                &transform_id.to_string(),
                &version.to_string(),
                &params_json.to_string(),
            );
            (state.qproperty_values(), track_id)
        };
        self.apply_values(values);
        QString::from(&track_id)
    }

    pub fn run_track(mut self: Pin<&mut Self>, track_id: QString) -> QString {
        let (values, job_id) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let job_id = state.run_track_state(&track_id.to_string());
            (state.qproperty_values(), job_id)
        };
        self.apply_values(values);
        QString::from(&job_id)
    }

    pub fn rerun_track(mut self: Pin<&mut Self>, track_id: QString) -> QString {
        let (values, job_id) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let job_id = state.rerun_track_state(&track_id.to_string());
            (state.qproperty_values(), job_id)
        };
        self.apply_values(values);
        QString::from(&job_id)
    }

    pub fn cancel_selected_job(mut self: Pin<&mut Self>) {
        let values = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            state.cancel_selected_job_state();
            state.qproperty_values()
        };
        self.apply_values(values);
    }

    pub fn refresh_cache_status(mut self: Pin<&mut Self>) -> QString {
        let (values, payload) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let invalid_refs = state.refresh_cache_status_state();
            (
                state.qproperty_values(),
                serde_json::to_string(&invalid_refs).unwrap_or_else(|_| "[]".to_string()),
            )
        };
        self.apply_values(values);
        QString::from(&payload)
    }

    pub fn add_manual_cue_track(mut self: Pin<&mut Self>, name: QString) -> QString {
        let (values, track_id) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let track_id = state.add_manual_cue_track_state(&name.to_string());
            (state.qproperty_values(), track_id)
        };
        self.apply_values(values);
        QString::from(&track_id)
    }

    pub fn create_editable_track_from_track(
        mut self: Pin<&mut Self>,
        source_track_id: QString,
    ) -> QString {
        let (values, track_id) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let track_id =
                state.create_editable_track_from_track_state(&source_track_id.to_string());
            (state.qproperty_values(), track_id)
        };
        self.apply_values(values);
        QString::from(&track_id)
    }

    pub fn set_track_expanded(mut self: Pin<&mut Self>, track_id: QString, expanded: bool) -> bool {
        let (values, changed) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let changed = state.set_track_expanded_state(&track_id.to_string(), expanded);
            (state.qproperty_values(), changed)
        };
        self.apply_values(values);
        changed
    }

    pub fn add_marker_to_selected_track_with_duration(
        mut self: Pin<&mut Self>,
        timestamp: f64,
        duration: f64,
        label: QString,
        category: QString,
        color: QString,
    ) -> QString {
        let (values, marker_id) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let marker_id = state.add_marker_to_selected_track_with_duration_state(
                timestamp,
                duration,
                &label.to_string(),
                &category.to_string(),
                &color.to_string(),
            );
            (state.qproperty_values(), marker_id)
        };
        self.apply_values(values);
        QString::from(&marker_id)
    }

    pub fn delete_marker_from_selected_track(mut self: Pin<&mut Self>, marker_id: QString) -> bool {
        let (values, deleted) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let deleted = state.delete_marker_from_selected_track_state(&marker_id.to_string());
            (state.qproperty_values(), deleted)
        };
        self.apply_values(values);
        deleted
    }

    pub fn delete_selected_markers(mut self: Pin<&mut Self>) -> i32 {
        let (values, deleted) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let deleted = state.delete_selected_markers_state();
            (state.qproperty_values(), deleted)
        };
        self.apply_values(values);
        deleted
    }

    pub fn update_selected_marker_with_duration(
        mut self: Pin<&mut Self>,
        timestamp: f64,
        duration: f64,
        label: QString,
        category: QString,
        color: QString,
    ) -> bool {
        let (values, updated) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let updated = state.update_selected_marker_with_duration_state(
                timestamp,
                duration,
                &label.to_string(),
                &category.to_string(),
                &color.to_string(),
            );
            (state.qproperty_values(), updated)
        };
        self.apply_values(values);
        updated
    }

    pub fn bulk_update_selected_markers(
        mut self: Pin<&mut Self>,
        label: QString,
        category: QString,
        color: QString,
    ) -> i32 {
        let (values, updated) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let updated = state.bulk_update_selected_markers_state(
                &label.to_string(),
                &category.to_string(),
                &color.to_string(),
            );
            (state.qproperty_values(), updated)
        };
        self.apply_values(values);
        updated
    }

    pub fn toggle_marker_selection(mut self: Pin<&mut Self>, marker_id: QString, additive: bool) {
        let values = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            state.toggle_marker_selection_state(&marker_id.to_string(), additive);
            state.qproperty_values()
        };
        self.apply_values(values);
    }

    pub fn move_selected_markers(
        mut self: Pin<&mut Self>,
        delta_seconds: f64,
        bypass_snap: bool,
    ) -> bool {
        let (values, moved) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let moved = state.move_selected_markers_state(delta_seconds, bypass_snap);
            (state.qproperty_values(), moved)
        };
        self.apply_values(values);
        moved
    }

    pub fn resize_marker(mut self: Pin<&mut Self>, marker_id: QString, duration: f64) -> bool {
        let (values, resized) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let resized = state.resize_marker_state(&marker_id.to_string(), duration);
            (state.qproperty_values(), resized)
        };
        self.apply_values(values);
        resized
    }

    pub fn undo(mut self: Pin<&mut Self>) -> bool {
        let (values, changed) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let changed = state.undo_state();
            (state.qproperty_values(), changed)
        };
        self.apply_values(values);
        changed
    }

    pub fn redo(mut self: Pin<&mut Self>) -> bool {
        let (values, changed) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let changed = state.redo_state();
            (state.qproperty_values(), changed)
        };
        self.apply_values(values);
        changed
    }

    pub fn play_selected_track(mut self: Pin<&mut Self>) -> bool {
        let (values, played) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let played = state.play_selected_track_state();
            (state.qproperty_values(), played)
        };
        self.apply_values(values);
        played
    }

    pub fn play_loaded_playback(mut self: Pin<&mut Self>) -> bool {
        let (values, played) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let played = state.play_loaded_playback_state();
            (state.qproperty_values(), played)
        };
        self.apply_values(values);
        played
    }

    pub fn pause_playback(mut self: Pin<&mut Self>) {
        let values = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            state.pause_playback_state();
            state.qproperty_values()
        };
        self.apply_values(values);
    }

    pub fn stop_playback(mut self: Pin<&mut Self>) {
        let values = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            state.stop_playback_state();
            state.qproperty_values()
        };
        self.apply_values(values);
    }

    pub fn seek_playback(mut self: Pin<&mut Self>, seconds: f64) {
        let values = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            state.seek_playback_state(seconds);
            state.qproperty_values()
        };
        self.apply_values(values);
    }

    pub fn nudge_playback(mut self: Pin<&mut Self>, delta_seconds: f64) {
        let values = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            state.nudge_playback_state(delta_seconds);
            state.qproperty_values()
        };
        self.apply_values(values);
    }

    pub fn set_playback_volume_invokable(mut self: Pin<&mut Self>, value: f64) {
        let values = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            state.set_playback_volume_state(value);
            state.qproperty_values()
        };
        self.apply_values(values);
    }

    pub fn set_timeline_zoom(mut self: Pin<&mut Self>, pixels_per_second: f64) {
        let values = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            state.set_timeline_zoom_state(pixels_per_second);
            state.qproperty_values()
        };
        self.apply_values(values);
    }

    pub fn set_timeline_scroll_seconds_invokable(mut self: Pin<&mut Self>, seconds: f64) {
        let values = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            state.set_timeline_scroll_seconds_state(seconds);
            state.qproperty_values()
        };
        self.apply_values(values);
    }

    pub fn set_timeline_visible_seconds_invokable(mut self: Pin<&mut Self>, seconds: f64) {
        let values = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            state.set_timeline_visible_seconds_state(seconds);
            state.qproperty_values()
        };
        self.apply_values(values);
    }

    pub fn snap_timeline_time(mut self: Pin<&mut Self>, seconds: f64, bypass_snap: bool) -> f64 {
        let (values, snapped) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let snapped = state.snap_timeline_time_state(seconds, bypass_snap);
            (state.qproperty_values(), snapped)
        };
        self.apply_values(values);
        snapped
    }

    fn apply_values(mut self: Pin<&mut Self>, values: ControllerPropertyValues) {
        self.as_mut().set_project_name(values.project_name);
        self.as_mut().set_project_path(values.project_path);
        self.as_mut().set_last_error(values.last_error);
        self.as_mut()
            .set_timeline_rows_json(values.timeline_rows_json);
        self.as_mut()
            .set_transform_specs_json(values.transform_specs_json);
        self.as_mut()
            .set_selected_track_id(values.selected_track_id);
        self.as_mut()
            .set_timeline_duration_seconds(values.timeline_duration_seconds);
        self.as_mut()
            .set_timeline_pixels_per_second(values.timeline_pixels_per_second);
        self.as_mut()
            .set_timeline_scroll_seconds(values.timeline_scroll_seconds);
        self.as_mut()
            .set_timeline_visible_seconds(values.timeline_visible_seconds);
        self.as_mut().set_is_dirty(values.is_dirty);
        self.as_mut()
            .set_selected_track_can_rerun(values.selected_track_can_rerun);
        self.as_mut()
            .set_selected_track_has_running_job(values.selected_track_has_running_job);
        self.as_mut()
            .set_selected_track_is_editable(values.selected_track_is_editable);
        self.as_mut()
            .set_selected_track_can_play(values.selected_track_can_play);
        self.as_mut()
            .set_selected_marker_ids_json(values.selected_marker_ids_json);
        self.as_mut()
            .set_selected_track_markers_json(values.selected_track_markers_json);
        self.as_mut()
            .set_marker_color_options_json(values.marker_color_options_json);
        self.as_mut().set_can_undo(values.can_undo);
        self.as_mut().set_can_redo(values.can_redo);
        self.as_mut()
            .set_playback_source_path(values.playback_source_path);
        self.as_mut()
            .set_playback_position_seconds(values.playback_position_seconds);
        self.as_mut()
            .set_playback_duration_seconds(values.playback_duration_seconds);
        self.as_mut()
            .set_playback_is_playing(values.playback_is_playing);
        self.as_mut()
            .set_playback_last_error(values.playback_last_error);
        self.set_playback_volume(values.playback_volume);
    }
}

fn job_registry() -> JobRegistry {
    let mut registry = JobRegistry::new();
    for spec in TransformRegistry::with_builtin_transforms()
        .specs()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>()
    {
        let transform_id = spec.id.clone();
        let register_result = if transform_id == "markers.fixed_interval" {
            registry.register(spec, fixed_interval_runner)
        } else {
            registry.register(spec, |_context, _params| Ok(TransformResult::default()))
        };
        register_result.expect("builtin job transforms are unique");
    }
    registry
}

fn fixed_interval_runner(
    context: &mut autolight_jobs::queue::TransformContext,
    params: &JsonObject,
) -> Result<TransformResult, TransformRunError> {
    if context.cancel_requested() {
        return Err(TransformRunError::Cancelled);
    }
    let duration = params
        .get("duration")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let interval = params
        .get("interval")
        .and_then(Value::as_f64)
        .unwrap_or(1.0);
    if !duration.is_finite() || duration < 0.0 {
        return Err(TransformRunError::Failed(
            "duration must be greater than or equal to zero".to_string(),
        ));
    }
    if !interval.is_finite() || interval <= 0.0 {
        return Err(TransformRunError::Failed(
            "interval must be greater than zero".to_string(),
        ));
    }

    let mut markers = Vec::new();
    let mut current = 0.0;
    while current <= duration + 1e-9 {
        if context.cancel_requested() {
            return Err(TransformRunError::Cancelled);
        }
        let mut marker = ProducedMarker::new(round6(current), "Beat");
        marker.category = "timing".to_string();
        marker.confidence = Some(1.0);
        marker
            .metadata
            .insert("interval".to_string(), serde_json::json!(interval));
        markers.push(marker);
        if duration > 0.0 {
            context.report_progress((current / duration).clamp(0.0, 1.0));
        }
        current += interval;
    }
    context.report_progress(1.0);
    Ok(TransformResult::markers(markers))
}

fn parse_params(params_json: &str) -> Result<JsonObject, String> {
    if params_json.trim().is_empty() {
        return Ok(JsonObject::new());
    }
    let value: Value = serde_json::from_str(params_json).map_err(|error| error.to_string())?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| "transform params must be a JSON object".to_string())
}

fn dependency_hash_for_new_track(
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

fn parent_compatibility_error(parent: &Track, spec: &TransformSpec) -> String {
    if spec.is_audio_input() {
        match parent.track_type {
            TrackType::Editable => "editable track has no source audio context".to_string(),
            _ => "parent track has no valid audio artifact".to_string(),
        }
    } else {
        "parent track is not compatible with transform".to_string()
    }
}

fn latest_active_job_id(project: &ProjectDocument, track_id: &str) -> Option<String> {
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

fn path_from_qml(path: &str) -> PathBuf {
    let value = path.trim();
    let path = value
        .strip_prefix("file://")
        .map(percent_decode)
        .unwrap_or_else(|| percent_decode(value));
    PathBuf::from(path)
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3]) {
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    decoded.push(byte);
                    index += 3;
                    continue;
                }
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).to_string()
}

fn with_autolight_suffix(path: PathBuf) -> PathBuf {
    if path.extension().and_then(|suffix| suffix.to_str()) == Some("autolight") {
        return path;
    }
    path.with_extension("autolight")
}

fn expanded_track_ids_from_project(project: &ProjectDocument) -> Option<BTreeSet<String>> {
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

fn selected_track_id_from_project(project: &ProjectDocument) -> String {
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

#[derive(Debug, Clone, PartialEq)]
struct AudioMetadata {
    duration: f64,
    sample_rate: u32,
    channels: u32,
}

fn probe_wav_file(path: &Path) -> Result<AudioMetadata, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("unsupported audio file: expected WAV".to_string());
    }

    let mut offset = 12;
    let mut sample_rate = 0_u32;
    let mut channels = 0_u32;
    let mut bits_per_sample = 0_u32;
    let mut data_bytes = 0_u32;

    while offset + 8 <= bytes.len() {
        let chunk_id = &bytes[offset..offset + 4];
        let chunk_size = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;
        offset += 8;
        if offset + chunk_size > bytes.len() {
            break;
        }
        if chunk_id == b"fmt " && chunk_size >= 16 {
            let audio_format = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
            if audio_format != 1 {
                return Err("unsupported WAV encoding: expected PCM".to_string());
            }
            channels = u16::from_le_bytes([bytes[offset + 2], bytes[offset + 3]]) as u32;
            sample_rate = u32::from_le_bytes([
                bytes[offset + 4],
                bytes[offset + 5],
                bytes[offset + 6],
                bytes[offset + 7],
            ]);
            bits_per_sample = u16::from_le_bytes([bytes[offset + 14], bytes[offset + 15]]) as u32;
        } else if chunk_id == b"data" {
            data_bytes = chunk_size as u32;
        }
        offset += chunk_size + (chunk_size % 2);
    }

    if sample_rate == 0 || channels == 0 || bits_per_sample == 0 {
        return Err("invalid WAV metadata".to_string());
    }
    let bytes_per_frame = channels * (bits_per_sample / 8);
    if bytes_per_frame == 0 {
        return Err("invalid WAV frame size".to_string());
    }
    Ok(AudioMetadata {
        duration: data_bytes as f64 / (sample_rate as f64 * bytes_per_frame as f64),
        sample_rate,
        channels,
    })
}

fn fingerprint_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Ok(format!("{hash:016x}"))
}

fn finite_non_negative(value: f64) -> f64 {
    if value.is_finite() && value >= 0.0 {
        value
    } else {
        0.0
    }
}

fn marker_color_options_json() -> String {
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

fn marker_color_key(marker: &Marker) -> &'static str {
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

fn marker_display_color_for_key(color_key: &str) -> &'static str {
    MARKER_COLOR_OPTIONS
        .iter()
        .find_map(|(key, _, color)| (*key == color_key).then_some(*color))
        .unwrap_or("#67e8f9")
}

fn json_string(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "[]".to_string())
}

fn json_object(values: impl IntoIterator<Item = (&'static str, Value)>) -> JsonObject {
    values
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

fn round6(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{AppControllerState, SMOKE_PROJECT_NAME};
    use autolight_core::project::{ResultState, TrackType};

    #[test]
    fn default_state_exposes_smoke_contract_and_transform_specs() {
        let state = AppControllerState::default();
        let specs: Value = serde_json::from_str(&state.transform_specs_json.to_string()).unwrap();

        assert_eq!(state.project_name.to_string(), SMOKE_PROJECT_NAME);
        assert!(state.last_error.to_string().is_empty());
        assert_eq!(state.timeline_rows_json.to_string(), "[]");
        assert_eq!(state.timeline_duration_seconds, 0.0);
        assert!(specs
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["transformId"] == "markers.fixed_interval"));
    }

    #[test]
    fn controller_loads_demo_project_and_selects_source_track() {
        let mut state = AppControllerState::default();

        state.load_demo_project_state();

        assert_eq!(state.project_name.to_string(), "Autolight Rust Demo");
        assert_eq!(state.selected_track_id.to_string(), "track_source");
        assert!(state
            .timeline_rows_json
            .to_string()
            .contains("track_source"));
        assert_eq!(state.timeline_duration_seconds, 2.0);
        assert!(!state.is_dirty);
    }

    #[test]
    fn controller_select_track_updates_selected_flags() {
        let mut state = AppControllerState::default();
        state.load_demo_project_state();

        state.select_track_state("track_edit");

        assert_eq!(state.selected_track_id.to_string(), "track_edit");
        assert!(state.selected_track_is_editable);
        assert!(!state.selected_track_has_running_job);
    }

    #[test]
    fn controller_add_transform_track_accepts_json_params_and_refreshes_rows() {
        let mut state = AppControllerState::default();
        state.load_demo_project_state();

        let track_id = state.add_transform_track_state(
            "track_source",
            "markers.fixed_interval",
            "1",
            r#"{"duration": 1.0, "interval": 0.5}"#,
        );

        assert!(!track_id.is_empty());
        assert_eq!(state.selected_track_id.to_string(), track_id);
        assert!(state.is_dirty);
        assert!(state.timeline_rows_json.to_string().contains(&track_id));
        let track = state
            .project
            .tracks
            .iter()
            .find(|track| track.id == track_id)
            .unwrap();
        assert_eq!(track.transform_id, "markers.fixed_interval");
        assert_eq!(track.transform_params["interval"], serde_json::json!(0.5));
    }

    #[test]
    fn controller_run_track_completes_fixed_interval_markers() {
        let mut state = AppControllerState::default();
        state.load_demo_project_state();
        let track_id = state.add_transform_track_state(
            "track_source",
            "markers.fixed_interval",
            "1",
            r#"{"duration": 1.0, "interval": 0.5}"#,
        );

        let job_id = state.run_track_state(&track_id);

        let track = state
            .project
            .tracks
            .iter()
            .find(|track| track.id == track_id)
            .unwrap();
        assert!(!job_id.is_empty());
        assert_eq!(track.result_state, ResultState::Complete);
        assert_eq!(
            state
                .project
                .markers
                .iter()
                .filter(|marker| marker.track_id == track_id)
                .count(),
            3
        );
        assert!(state
            .timeline_rows_json
            .to_string()
            .contains("\"markerCount\":3"));
    }

    #[test]
    fn controller_rejects_audio_transform_for_generated_marker_parent_without_audio_artifact() {
        let mut state = AppControllerState::default();
        state.load_demo_project_state();

        let track_id =
            state.add_transform_track_state("track_beats", "waveform.summary", "1", "{}");

        assert!(track_id.is_empty());
        assert!(state
            .last_error
            .to_string()
            .contains("parent track has no valid audio artifact"));
    }

    #[test]
    fn controller_refresh_cache_status_marks_invalid_refs_stale() {
        let mut state = AppControllerState::default();
        state.load_demo_project_state();
        state
            .project
            .cache_entries
            .iter_mut()
            .find(|entry| entry.id == "cache_energy")
            .unwrap()
            .validation_status = "invalid".to_string();

        let invalid = state.refresh_cache_status_state();

        assert_eq!(invalid, ["cache_energy"]);
        assert!(state
            .last_error
            .to_string()
            .contains("invalid cache artifacts"));
        assert_eq!(
            state
                .project
                .tracks
                .iter()
                .find(|track| track.id == "track_drum_energy")
                .unwrap()
                .result_state,
            ResultState::Stale
        );
    }

    #[test]
    fn controller_tracks_selected_marker_ids_and_payloads() {
        let mut state = AppControllerState::default();
        state.load_demo_project_state();
        state.select_track_state("track_edit");

        state.toggle_marker_selection_state("marker_edit_1", false);
        state.toggle_marker_selection_state("marker_edit_2", true);

        let selected_ids: Vec<String> =
            serde_json::from_str(&state.selected_marker_ids_json.to_string()).unwrap();
        let markers = json_array(&state.selected_track_markers_json.to_string());
        let rows = json_array(&state.timeline_rows_json.to_string());
        let editable_row = rows
            .iter()
            .find(|row| row["trackId"] == "track_edit")
            .unwrap();

        assert_eq!(selected_ids, ["marker_edit_1", "marker_edit_2"]);
        assert_eq!(
            markers[0]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>(),
            [
                "category".to_string(),
                "color".to_string(),
                "colorKey".to_string(),
                "duration".to_string(),
                "id".to_string(),
                "label".to_string(),
                "selected".to_string(),
                "timestamp".to_string()
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(markers[0]["colorKey"], "amber");
        assert_eq!(markers[0]["color"], "#fbbf24");
        assert_eq!(markers[0]["selected"], true);
        assert_eq!(markers[1]["selected"], true);
        assert_eq!(editable_row["markerSpans"][0]["selected"], true);
    }

    #[test]
    fn controller_edits_selected_markers_roundtrip() {
        let mut state = AppControllerState::default();
        state.load_demo_project_state();
        state.select_track_state("track_edit");

        let marker_id = state
            .add_marker_to_selected_track_with_duration_state(1.25, 0.5, "Blackout", "cue", "cyan");
        assert!(!marker_id.is_empty());
        assert_eq!(state.selected_marker_ids, [marker_id.clone()]);

        assert!(state
            .update_selected_marker_with_duration_state(1.5, 0.75, "Scene", "lighting", "violet",));
        assert!(state.move_selected_markers_state(0.25, true));
        assert!(state.resize_marker_state(&marker_id, 1.0));
        let marker = state
            .project
            .markers
            .iter()
            .find(|marker| marker.id == marker_id)
            .unwrap();
        assert_eq!(marker.timestamp, 1.75);
        assert_eq!(marker.duration, Some(1.0));
        assert_eq!(marker.label, "Scene");
        assert_eq!(marker.category, "lighting");
        assert_eq!(marker.metadata["color"], serde_json::json!("violet"));

        assert!(state.delete_marker_from_selected_track_state(&marker_id));
        assert!(state
            .project
            .markers
            .iter()
            .all(|marker| marker.id != marker_id));
        assert!(state.selected_marker_ids.is_empty());
    }

    #[test]
    fn controller_derives_editable_track_from_marker_track() {
        let mut state = AppControllerState::default();
        state.load_demo_project_state();

        let track_id = state.create_editable_track_from_track_state("track_beats");

        let track = state
            .project
            .tracks
            .iter()
            .find(|track| track.id == track_id)
            .unwrap();
        assert_eq!(track.track_type, TrackType::Editable);
        assert_eq!(track.input_track_ids, ["track_beats"]);
        assert_eq!(track.provenance["source_track_id"], "track_beats");
        assert_eq!(state.selected_track_id.to_string(), track_id);
        assert_eq!(
            state
                .project
                .markers
                .iter()
                .filter(|marker| marker.track_id == track_id)
                .count(),
            3
        );
        assert!(state.can_undo);
        assert!(state.is_dirty);
    }

    #[test]
    fn controller_undo_redo_reconciles_dirty_and_selection_state() {
        let mut state = AppControllerState::default();
        state.load_demo_project_state();
        state.select_track_state("track_edit");

        let marker_id = state
            .add_marker_to_selected_track_with_duration_state(1.25, 0.5, "Blackout", "cue", "cyan");

        assert!(state.can_undo);
        assert!(!state.can_redo);
        assert!(state.is_dirty);
        assert_eq!(state.selected_marker_ids, [marker_id.clone()]);

        assert!(state.undo_state());
        assert!(!state
            .project
            .markers
            .iter()
            .any(|marker| marker.id == marker_id));
        assert!(state.selected_marker_ids.is_empty());
        assert!(!state.can_undo);
        assert!(state.can_redo);
        assert!(!state.is_dirty);

        assert!(state.redo_state());
        assert!(state
            .project
            .markers
            .iter()
            .any(|marker| marker.id == marker_id));
        assert!(!state.can_redo);
        assert!(state.is_dirty);
    }

    #[test]
    fn controller_collapses_tree_rows_and_reselects_visible_parent() {
        let mut state = AppControllerState::default();
        state.load_demo_project_state();
        state.select_track_state("track_drum_energy");

        assert!(state.set_track_expanded_state("track_drums", false));

        let rows = json_array(&state.timeline_rows_json.to_string());
        assert!(!rows.iter().any(|row| row["trackId"] == "track_drum_energy"));
        let drums = rows
            .iter()
            .find(|row| row["trackId"] == "track_drums")
            .unwrap();
        assert_eq!(drums["expanded"], false);
        assert_eq!(state.selected_track_id.to_string(), "track_drums");
        assert!(state.is_dirty);
    }

    #[test]
    fn controller_import_audio_adds_source_track_and_playability() {
        let root = test_dir("import-audio");
        let audio_path = root.join("song.wav");
        write_test_wav(&audio_path, 8_000, 1, 16_000);
        let mut state = AppControllerState::default();

        let track_id = state.import_audio_state(audio_path.to_str().unwrap());

        assert!(!track_id.is_empty());
        assert_eq!(state.selected_track_id.to_string(), track_id);
        assert!(state.selected_track_can_play);
        assert!(state.is_dirty);
        assert_eq!(state.project.audio_assets.len(), 1);
        assert_eq!(state.project.audio_assets[0].duration, 2.0);
        assert_eq!(state.project.audio_assets[0].sample_rate, 8_000);
        assert_eq!(state.project.audio_assets[0].channels, 1);
        assert_eq!(state.project.tracks[0].track_type, TrackType::Source);
        assert_eq!(
            state.project.tracks[0].provenance["asset_id"],
            "asset_rust_0001"
        );
    }

    #[test]
    fn controller_save_and_open_project_roundtrip_updates_path_and_clean_state() {
        let root = test_dir("save-open");
        let audio_path = root.join("song.wav");
        let project_path = root.join("show");
        let saved_path = root.join("show.autolight");
        write_test_wav(&audio_path, 8_000, 1, 8_000);
        let mut state = AppControllerState::default();
        let track_id = state.import_audio_state(audio_path.to_str().unwrap());

        assert!(state.save_project_state(project_path.to_str().unwrap()));
        assert!(saved_path.is_file());
        assert_eq!(state.project_path.to_string(), saved_path.to_string_lossy());
        assert!(!state.is_dirty);

        let mut opened = AppControllerState::default();
        assert!(opened.open_project_state(saved_path.to_str().unwrap()));

        assert_eq!(
            opened.project_path.to_string(),
            saved_path.to_string_lossy()
        );
        assert_eq!(opened.selected_track_id.to_string(), track_id);
        assert!(opened.selected_track_can_play);
        assert!(!opened.is_dirty);
        assert!(opened.timeline_rows_json.to_string().contains(&track_id));
    }

    #[test]
    fn controller_playback_state_transitions_from_selected_track() {
        let root = test_dir("playback");
        let audio_path = root.join("song.wav");
        write_test_wav(&audio_path, 8_000, 1, 16_000);
        let mut state = AppControllerState::default();
        state.import_audio_state(audio_path.to_str().unwrap());

        assert!(state.play_selected_track_state());
        assert_eq!(
            state.playback_source_path.to_string(),
            audio_path.to_string_lossy()
        );
        assert_eq!(state.playback_duration_seconds, 2.0);
        assert!(state.playback_is_playing);

        state.seek_playback_state(20.0);
        assert_eq!(state.playback_position_seconds, 2.0);
        state.nudge_playback_state(-0.75);
        assert_eq!(state.playback_position_seconds, 1.25);
        state.set_playback_volume_state(2.0);
        assert_eq!(state.playback_volume, 1.0);
        state.pause_playback_state();
        assert!(!state.playback_is_playing);
        assert!(state.play_loaded_playback_state());
        assert!(state.playback_is_playing);
        state.stop_playback_state();
        assert!(!state.playback_is_playing);
        assert_eq!(state.playback_position_seconds, 0.0);
    }

    #[test]
    fn controller_persists_timeline_viewport_state() {
        let root = test_dir("viewport");
        let audio_path = root.join("song.wav");
        let project_path = root.join("show.autolight");
        write_test_wav(&audio_path, 8_000, 1, 120_000);
        let mut state = AppControllerState::default();
        state.import_audio_state(audio_path.to_str().unwrap());
        state.set_timeline_visible_seconds_state(4.0);
        state.set_timeline_zoom_state(144.0);
        state.set_timeline_scroll_seconds_state(3.0);

        assert!(state.save_project_state(project_path.to_str().unwrap()));
        let mut reopened = AppControllerState::default();
        assert!(reopened.open_project_state(project_path.to_str().unwrap()));

        assert_eq!(reopened.timeline_pixels_per_second, 144.0);
        assert_eq!(reopened.timeline_scroll_seconds, 3.0);
        assert_eq!(
            reopened.project.ui_state["timeline"]["pixels_per_second"],
            serde_json::json!(144.0)
        );
        assert_eq!(
            reopened.project.ui_state["timeline"]["scroll_seconds"],
            serde_json::json!(3.0)
        );
    }

    #[test]
    fn controller_snaps_single_marker_moves_to_visible_timing_markers() {
        let mut state = AppControllerState::default();
        state.load_demo_project_state();
        state.select_track_state("track_edit");
        state.toggle_marker_selection_state("marker_edit_1", false);

        assert_eq!(state.snap_timeline_time_state(0.53, false), 0.5);
        assert_eq!(state.snap_timeline_time_state(0.53, true), 0.53);
        assert!(state.move_selected_markers_state(0.53, false));
        let marker = state
            .project
            .markers
            .iter()
            .find(|marker| marker.id == "marker_edit_1")
            .unwrap();
        assert_eq!(marker.timestamp, 0.5);

        assert!(state.undo_state());
        assert!(state.move_selected_markers_state(0.53, true));
        let marker = state
            .project
            .markers
            .iter()
            .find(|marker| marker.id == "marker_edit_1")
            .unwrap();
        assert_eq!(marker.timestamp, 0.53);
    }

    #[test]
    fn qml_rust_adapter_uses_controller_models_and_actions() {
        let qml = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../UI/Main.qml"),
        )
        .unwrap();

        assert!(qml.contains("rustController.transformSpecsJson"));
        assert!(qml.contains("rustController.selectedTrackId"));
        assert!(qml.contains("rustController.addTransformTrack"));
        assert!(qml.contains("rustController.runTrack"));
        assert!(qml.contains("rustController.selectedMarkerIdsJson"));
        assert!(qml.contains("rustController.selectedTrackMarkersJson"));
        assert!(qml.contains("rustController.markerColorOptionsJson"));
        assert!(qml.contains("rustController.addMarkerToSelectedTrackWithDuration"));
        assert!(qml.contains("rustController.updateSelectedMarkerWithDuration"));
        assert!(qml.contains("rustController.bulkUpdateSelectedMarkers"));
        assert!(qml.contains("rustController.toggleMarkerSelection"));
        assert!(qml.contains("rustController.createEditableTrackFromTrack"));
        assert!(qml.contains("rustController.setTrackExpanded"));
        assert!(qml.contains("rustController.undo"));
        assert!(qml.contains("rustController.redo"));
        assert!(qml.contains("rustController.projectPath"));
        assert!(qml.contains("rustController.selectedTrackCanPlay"));
        assert!(qml.contains("rustController.openProject"));
        assert!(qml.contains("rustController.saveProject"));
        assert!(qml.contains("rustController.importAudio"));
        assert!(qml.contains("rustController.playSelectedTrack"));
        assert!(qml.contains("rustController.playbackSourcePath"));
        assert!(qml.contains("rustController.playbackPositionSeconds"));
        assert!(qml.contains("rustController.playbackDurationSeconds"));
        assert!(qml.contains("rustController.playbackIsPlaying"));
        assert!(qml.contains("rustController.playbackLastError"));
        assert!(qml.contains("rustController.playbackVolume"));
        assert!(qml.contains("rustController.setPlaybackVolumeValue"));
        assert!(qml.contains("rustController.timelinePixelsPerSecond"));
        assert!(qml.contains("rustController.timelineScrollSeconds"));
        assert!(qml.contains("rustController.timelineVisibleSeconds"));
        assert!(qml.contains("rustController.setTimelineZoom"));
        assert!(qml.contains("rustController.applyTimelineScrollSeconds"));
        assert!(qml.contains("rustController.applyTimelineVisibleSeconds"));
        assert!(qml.contains("rustController.snapTimelineTime"));
        assert!(qml.contains("function add_fixed_interval_track(trackId, duration, interval) { return add_transform_track"));
        assert!(
            qml.contains("function add_vocals_stem_track(trackId) { return add_transform_track")
        );
        assert!(qml.contains("transformModel.append"));
        assert!(qml.contains("function version_at(index)"));
    }

    fn json_array(payload: &str) -> Vec<Value> {
        serde_json::from_str(payload).unwrap()
    }

    fn test_dir(name: &str) -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "autolight-qt-{name}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_test_wav(path: &std::path::Path, sample_rate: u32, channels: u16, frames: u32) {
        use std::io::Write;

        let bits_per_sample = 16_u16;
        let bytes_per_sample = u32::from(bits_per_sample / 8);
        let data_bytes = frames * u32::from(channels) * bytes_per_sample;
        let byte_rate = sample_rate * u32::from(channels) * bytes_per_sample;
        let block_align = channels * (bits_per_sample / 8);
        let mut file = std::fs::File::create(path).unwrap();
        file.write_all(b"RIFF").unwrap();
        file.write_all(&(36 + data_bytes).to_le_bytes()).unwrap();
        file.write_all(b"WAVE").unwrap();
        file.write_all(b"fmt ").unwrap();
        file.write_all(&16_u32.to_le_bytes()).unwrap();
        file.write_all(&1_u16.to_le_bytes()).unwrap();
        file.write_all(&channels.to_le_bytes()).unwrap();
        file.write_all(&sample_rate.to_le_bytes()).unwrap();
        file.write_all(&byte_rate.to_le_bytes()).unwrap();
        file.write_all(&block_align.to_le_bytes()).unwrap();
        file.write_all(&bits_per_sample.to_le_bytes()).unwrap();
        file.write_all(b"data").unwrap();
        file.write_all(&data_bytes.to_le_bytes()).unwrap();
        file.write_all(&vec![0_u8; data_bytes as usize]).unwrap();
    }
}
