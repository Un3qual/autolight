use core::pin::Pin;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use autolight_analysis::waveform::visible_samples_from_json;
use autolight_core::cache::cache_entry_for_bytes;
use autolight_core::graph::{default_expanded_track_ids, find_track, mark_dependents_stale};
use autolight_core::history::{
    DependentTrackSnapshot, EditHistory, MarkerSnapshotCommand, ProjectSnapshotCommand,
};
use autolight_core::markers::{
    add_editable_marker, bulk_update_editable_markers, create_manual_editable_track,
    delete_editable_marker, move_editable_markers, resize_editable_marker, update_editable_marker,
    BulkMarkerUpdate, EditableMarkerInput, MarkerUpdate,
};
use autolight_core::project::{
    AudioAsset, CacheValidationStatus, ImportStatus, JsonObject, Marker, ProjectDocument,
    ResultState, Track, TrackType,
};
use autolight_core::transforms::TransformRegistry;
use autolight_jobs::queue::LocalJobQueue;
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use serde_json::{json, Value};

use crate::timeline_model::{rust_demo_project, RUST_DEMO_PROJECT_NAME};
use crate::transform_model::transform_specs_json;

mod audio;
mod job_worker;
mod jobs;
mod markers;
mod playback_controller;
mod project_io;
mod project_state;
#[cfg(test)]
mod tests;
mod timeline_controller;

#[cfg(test)]
use audio::WAVE_SUBFORMAT_PCM;
use audio::{inspect_wav_file, write_silent_wav};
use job_worker::{new_shared_job_progress, spawn_job_worker, JobWorker, SharedJobProgress};
use jobs::job_registry;
use markers::{
    finite_duration, finite_non_negative, is_timing_snap_category, json_object, json_string,
    marker_color_key, marker_color_options_json, marker_display_color_for_key, marker_end_seconds,
};
use playback_controller::PlaybackControllerState;
use project_io::{
    audio_asset_load_error, audio_asset_project_dir_relink_path, cache_entry_is_valid,
    cache_entry_path_is_safe, current_project_dir, path_from_qml, with_autolight_suffix,
};
use project_state::{
    clear_waveform_provenance, dependency_hash_for_new_track, expanded_track_ids_from_project,
    is_audio_dependency_error, latest_active_job_id, parent_compatibility_error, parse_params,
    restore_audio_dependency_dependents, selected_track_id_from_project, track_inputs_are_complete,
};
use timeline_controller::TimelineControllerState;

const SMOKE_PROJECT_NAME: &str = "Autolight Rust Smoke";
const TIMELINE_DEFAULT_PIXELS_PER_SECOND: f64 = 96.0;
const TIMELINE_MIN_PIXELS_PER_SECOND: f64 = 24.0;
const TIMELINE_MAX_PIXELS_PER_SECOND: f64 = 240.0;
const TIMELINE_DEFAULT_VISIBLE_SECONDS: f64 = 8.0;
const TIMELINE_MIN_VISIBLE_SECONDS: f64 = 0.01;
const SNAP_THRESHOLD_PIXELS: f64 = 10.0;
static NEXT_DEMO_TEMP_DIR: AtomicU64 = AtomicU64::new(1);

pub struct AppControllerState {
    project_name: QString,
    project_path: QString,
    last_error: QString,
    timeline_rows_json: QString,
    transform_specs_json: QString,
    selected_track_id: QString,
    timeline: TimelineControllerState,
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
    visible_track_range: Option<(usize, usize)>,
    visible_track_ids: BTreeSet<String>,
    can_undo: bool,
    can_redo: bool,
    playback: PlaybackControllerState,
    playback_source_path: QString,
    playback_position_seconds: f64,
    playback_duration_seconds: f64,
    playback_is_playing: bool,
    playback_last_error: QString,
    playback_volume: f64,
    project: ProjectDocument,
    transform_registry: TransformRegistry,
    job_queue: LocalJobQueue,
    job_workers: Vec<JobWorker>,
    job_progress: SharedJobProgress,
    next_track_number: u64,
    next_asset_number: u64,
    selected_marker_ids: Vec<String>,
    expanded_track_ids: BTreeSet<String>,
    edit_history: EditHistory,
    non_history_dirty: bool,
    demo_temp_dir: DemoTempDir,
}

#[derive(Debug, Clone, PartialEq)]
struct MarkerEditSnapshot {
    track_id: String,
    markers: Vec<Marker>,
    dependents: Vec<DependentTrackSnapshot>,
}

impl Default for AppControllerState {
    fn default() -> Self {
        let transform_registry = TransformRegistry::with_builtin_transforms();
        let transform_specs =
            transform_specs_json(&transform_registry).unwrap_or_else(|_| "[]".to_string());
        let timeline = TimelineControllerState::default();
        let playback = PlaybackControllerState::default();
        Self {
            project_name: QString::from(SMOKE_PROJECT_NAME),
            project_path: QString::default(),
            last_error: QString::default(),
            timeline_rows_json: QString::from("[]"),
            transform_specs_json: QString::from(&transform_specs),
            selected_track_id: QString::default(),
            timeline_duration_seconds: timeline.duration_seconds(),
            timeline_pixels_per_second: timeline.pixels_per_second(),
            timeline_scroll_seconds: timeline.scroll_seconds(),
            timeline_visible_seconds: timeline.visible_seconds(),
            is_dirty: false,
            selected_track_can_rerun: false,
            selected_track_has_running_job: false,
            selected_track_is_editable: false,
            selected_track_can_play: false,
            selected_marker_ids_json: QString::from("[]"),
            selected_track_markers_json: QString::from("[]"),
            marker_color_options_json: QString::from(&marker_color_options_json()),
            visible_track_range: None,
            visible_track_ids: BTreeSet::new(),
            can_undo: false,
            can_redo: false,
            playback_source_path: playback.source_path().clone(),
            playback_position_seconds: playback.position_seconds(),
            playback_duration_seconds: playback.duration_seconds(),
            playback_is_playing: playback.is_playing(),
            playback_last_error: playback.last_error().clone(),
            playback_volume: playback.volume(),
            project: ProjectDocument::new("project_empty", SMOKE_PROJECT_NAME),
            transform_registry,
            job_queue: LocalJobQueue::new(job_registry()),
            job_workers: Vec::default(),
            job_progress: new_shared_job_progress(),
            next_track_number: 1,
            next_asset_number: 1,
            selected_marker_ids: Vec::default(),
            expanded_track_ids: BTreeSet::new(),
            edit_history: EditHistory::new(),
            non_history_dirty: false,
            demo_temp_dir: DemoTempDir::default(),
            timeline,
            playback,
        }
    }
}

#[derive(Default)]
struct DemoTempDir {
    path: Option<PathBuf>,
}

impl DemoTempDir {
    fn replace(&mut self, path: PathBuf) {
        self.clear();
        self.path = Some(path);
    }

    fn clear(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_dir_all(path);
        }
    }

    fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

impl Drop for DemoTempDir {
    fn drop(&mut self) {
        self.clear();
    }
}

impl AppControllerState {
    fn load_demo_project_state(&mut self) {
        self.reset_job_runtime_state();
        self.clear_demo_temp_dir();
        self.project = rust_demo_project();
        let demo_setup_error = self.prepare_demo_audio_asset().err();
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
        self.reset_history_clean();
        self.refresh_view_state();
        if let Some(error) = demo_setup_error {
            self.set_error(error);
        } else {
            self.last_error = QString::default();
        }
    }

    fn clear_project_state(&mut self) {
        self.reset_job_runtime_state();
        self.clear_demo_temp_dir();
        self.project = ProjectDocument::new("project_empty", SMOKE_PROJECT_NAME);
        self.project_name = QString::from(SMOKE_PROJECT_NAME);
        self.project_path = QString::default();
        self.selected_track_id = QString::default();
        self.selected_marker_ids.clear();
        self.expanded_track_ids.clear();
        self.unload_playback();
        self.reset_timeline_view_state();
        self.reset_history_clean();
        self.last_error = QString::default();
        self.refresh_view_state();
    }

    fn prepare_demo_audio_asset(&mut self) -> Result<(), String> {
        let demo_dir = self.create_demo_temp_dir()?;
        let audio_path = demo_dir.join("rust-demo.wav");
        write_silent_wav(&audio_path, 8_000, 1, 16_000)?;
        let inspection = inspect_wav_file(&audio_path)?;
        let asset = self
            .project
            .audio_assets
            .iter_mut()
            .find(|asset| asset.id == "asset_demo")
            .ok_or_else(|| "demo audio asset missing".to_string())?;
        asset.path = audio_path.to_string_lossy().to_string();
        asset.duration = inspection.metadata.duration;
        asset.sample_rate = inspection.metadata.sample_rate;
        asset.channels = inspection.metadata.channels;
        asset.fingerprint = inspection.fingerprint;
        asset.import_status = ImportStatus::Online;
        asset.relink_hint.clear();
        self.prepare_demo_cache_artifacts(&demo_dir)?;
        Ok(())
    }

    fn prepare_demo_cache_artifacts(&mut self, demo_dir: &Path) -> Result<(), String> {
        let waveform_payload = self
            .project
            .tracks
            .iter()
            .find(|track| track.id == "track_waveform")
            .and_then(|track| track.provenance.get("waveform_payload"))
            .cloned()
            .ok_or_else(|| "demo waveform payload missing".to_string())?;
        let waveform_payload =
            serde_json::to_vec(&waveform_payload).map_err(|error| error.to_string())?;
        self.materialize_demo_cache_artifact(
            demo_dir,
            "track_waveform",
            "cache_waveform",
            "waveform",
            "dep_waveform",
            &waveform_payload,
        )?;
        let stem_payload = fs::read(demo_dir.join("rust-demo.wav")).map_err(|error| {
            format!(
                "failed to read demo stem payload {}: {error}",
                demo_dir.join("rust-demo.wav").display()
            )
        })?;
        self.materialize_demo_cache_artifact(
            demo_dir,
            "track_drums",
            "cache_drums",
            "stem",
            "dep_drums",
            &stem_payload,
        )?;
        let energy_payload = br#"{"duration":2.0,"samples":[{"time":0.0,"energy":0.24},{"time":1.0,"energy":0.67}]}"#;
        self.materialize_demo_cache_artifact(
            demo_dir,
            "track_drum_energy",
            "cache_energy",
            "energy",
            "dep_energy",
            energy_payload,
        )?;
        Ok(())
    }

    fn materialize_demo_cache_artifact(
        &mut self,
        demo_dir: &Path,
        track_id: &str,
        placeholder_cache_id: &str,
        artifact_kind: &str,
        dependency_hash: &str,
        payload: &[u8],
    ) -> Result<(), String> {
        let entry = cache_entry_for_bytes(artifact_kind, dependency_hash, payload, "1", "demo")
            .map_err(|error| error.to_string())?;
        let payload_path = demo_dir.join(&entry.path);
        if let Some(parent) = payload_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(&payload_path, payload).map_err(|error| error.to_string())?;
        self.project
            .cache_entries
            .retain(|candidate| candidate.id != placeholder_cache_id);
        self.project.cache_entries.push(entry.clone());
        if let Some(track) = self
            .project
            .tracks
            .iter_mut()
            .find(|track| track.id == track_id)
        {
            track.cache_refs = vec![entry.id];
        }
        Ok(())
    }

    fn create_demo_temp_dir(&mut self) -> Result<PathBuf, String> {
        self.clear_demo_temp_dir();
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let sequence = NEXT_DEMO_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "autolight-rust-demo-{}-{suffix}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).map_err(|error| error.to_string())?;
        self.demo_temp_dir.replace(path.clone());
        Ok(path)
    }

    fn clear_demo_temp_dir(&mut self) {
        self.demo_temp_dir.clear();
    }

    fn reset_job_runtime_state(&mut self) {
        let active_job_ids = self
            .job_workers
            .iter()
            .map(|worker| worker.job_id().to_string())
            .collect::<Vec<_>>();
        for job_id in &active_job_ids {
            let _ = self.job_queue.cancel(job_id);
        }
        let mut join_error = None;
        for worker in self.job_workers.drain(..) {
            if let Err(error) = worker.join() {
                join_error = Some(error);
            }
        }
        if let Some(error) = join_error {
            self.last_error = QString::from(error);
        }
        self.cancel_project_runs_for_reset(&active_job_ids);
        self.job_queue = LocalJobQueue::new(job_registry());
        if let Ok(mut progress) = self.job_progress.lock() {
            progress.clear();
        }
    }

    fn cancel_project_runs_for_reset(&mut self, job_ids: &[String]) {
        let error = "job cancelled because project runtime reset";
        let mut affected_track_ids = Vec::new();
        for run in &mut self.project.job_runs {
            if job_ids.contains(&run.id)
                && matches!(run.state, ResultState::Pending | ResultState::Running)
            {
                run.state = ResultState::Cancelled;
                run.progress = 1.0;
                run.error = error.to_string();
                affected_track_ids.push(run.track_id.clone());
            }
        }
        for track_id in affected_track_ids {
            if let Some(track) = self
                .project
                .tracks
                .iter_mut()
                .find(|track| track.id == track_id)
            {
                track.result_state = ResultState::Cancelled;
                track.error = error.to_string();
            }
        }
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
                return String::default();
            }
        };
        let Some(parent) = find_track(&self.project, parent_track_id) else {
            self.set_error(format!("track not found: {parent_track_id}"));
            return String::default();
        };
        if parent.result_state != ResultState::Complete {
            self.set_error(format!("parent track is not complete: {}", parent.name));
            return String::default();
        }
        let spec = match self.transform_registry.get(transform_id, Some(version)) {
            Ok(spec) => spec.clone(),
            Err(error) => {
                self.set_error(error.to_string());
                return String::default();
            }
        };
        if !self.job_queue.has_runner(transform_id, version) {
            self.set_error(format!(
                "transform is not available in the Rust runtime: {transform_id}"
            ));
            return String::default();
        }
        if !spec.is_compatible_parent(&self.project, parent_track_id) {
            self.set_error(parent_compatibility_error(parent, &spec));
            return String::default();
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
                return String::default();
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
            output_schema: spec.output_schema.to_string(),
            dependency_hash,
            result_state: ResultState::Pending,
            cache_refs: Vec::default(),
            provenance: JsonObject::default(),
            error: String::default(),
        });
        self.expand_parent_for_new_child(parent_track_id);
        self.selected_track_id = QString::from(&track_id);
        self.selected_marker_ids.clear();
        self.mark_project_mutation_dirty();
        self.last_error = QString::default();
        self.refresh_view_state();
        track_id
    }

    #[cfg(test)]
    fn run_track_state(&mut self, track_id: &str) -> String {
        let runtime_params = match self.runtime_params_for_track_run(track_id) {
            Ok(params) => params,
            Err(error) => {
                self.set_error(error);
                return String::default();
            }
        };
        if let Err(error) = self.ensure_artifact_dir_for_track_run(track_id) {
            self.set_error(error);
            self.refresh_view_state();
            return String::default();
        }
        match self
            .job_queue
            .submit_with_runtime_params(&mut self.project, track_id, runtime_params)
        {
            Ok(job_id) => {
                let artifact_dir = self.current_artifact_dir();
                if let Err(error) = self
                    .job_queue
                    .run_next_with_artifact_dir(&mut self.project, artifact_dir.as_deref())
                {
                    self.mark_project_mutation_dirty();
                    self.set_error(error.to_string());
                    self.refresh_view_state();
                    return String::default();
                }
                let waveform_error = self
                    .refresh_waveform_track_provenance(track_id, artifact_dir.as_deref())
                    .err();
                self.mark_project_mutation_dirty();
                if let Some(error) = waveform_error {
                    self.set_error(error);
                } else {
                    self.last_error = QString::default();
                }
                self.refresh_view_state();
                job_id
            }
            Err(error) => {
                self.set_error(error.to_string());
                String::default()
            }
        }
    }

    fn submit_track_state(&mut self, track_id: &str) -> String {
        let runtime_params = match self.runtime_params_for_track_run(track_id) {
            Ok(params) => params,
            Err(error) => {
                self.set_error(error);
                return String::default();
            }
        };
        if let Err(error) = self.ensure_artifact_dir_for_track_run(track_id) {
            self.set_error(error);
            self.refresh_view_state();
            return String::default();
        }
        let job_id = match self.job_queue.submit_with_runtime_params(
            &mut self.project,
            track_id,
            runtime_params,
        ) {
            Ok(job_id) => job_id,
            Err(error) => {
                self.set_error(error.to_string());
                return String::default();
            }
        };
        let token = match self.job_queue.detach_pending_job(&job_id) {
            Ok(token) => token,
            Err(error) => {
                self.set_error(error.to_string());
                return String::default();
            }
        };
        let artifact_dir = self.current_artifact_dir();
        let worker = spawn_job_worker(
            self.project.clone(),
            job_id.clone(),
            artifact_dir,
            token,
            self.job_progress.clone(),
        );
        self.job_workers.push(worker);
        self.mark_project_mutation_dirty();
        self.last_error = QString::default();
        self.refresh_view_state();
        job_id
    }

    fn poll_job_workers_state(&mut self) -> i32 {
        let mut changed = self.apply_worker_progress();
        let mut index = 0;
        while index < self.job_workers.len() {
            if !self.job_workers[index].is_finished() {
                index += 1;
                continue;
            }
            let worker = self.job_workers.swap_remove(index);
            let job_id = worker.job_id().to_string();
            match worker.join() {
                Ok(result) => {
                    self.merge_job_worker_result(result);
                }
                Err(error) => {
                    self.finalize_worker_join_error(&job_id, &error);
                    self.set_error(error);
                }
            }
            self.job_queue.forget_cancellation_token(&job_id);
            changed += 1;
        }
        if changed > 0 {
            self.mark_non_history_dirty();
            self.refresh_view_state();
        } else {
            self.refresh_selected_state();
        }
        changed
    }

    fn apply_worker_progress(&mut self) -> i32 {
        let Ok(progress) = self.job_progress.lock() else {
            return 0;
        };
        let mut changed = 0;
        for run in &mut self.project.job_runs {
            let Some(value) = progress.get(&run.id).copied() else {
                continue;
            };
            if (run.progress - value).abs() > 1e-9 {
                run.progress = value;
                changed += 1;
            }
        }
        changed
    }

    fn ensure_artifact_dir_for_track_run(&self, track_id: &str) -> Result<(), String> {
        if self.current_artifact_dir().is_some() {
            return Ok(());
        }
        let Some(track) = find_track(&self.project, track_id) else {
            return Ok(());
        };
        let spec = self
            .transform_registry
            .get(&track.transform_id, Some(&track.transform_version))
            .map_err(|error| error.to_string())?;
        if spec.output_schema.as_str().starts_with("artifact.") {
            Err("save the project before running artifact-producing transforms".to_string())
        } else {
            Ok(())
        }
    }

    fn finalize_worker_join_error(&mut self, job_id: &str, error: &str) {
        let run_index = self
            .project
            .job_runs
            .iter()
            .position(|run| run.id == job_id);
        if let Some(run_index) = run_index {
            if matches!(
                self.project.job_runs[run_index].state,
                ResultState::Pending | ResultState::Running
            ) {
                self.project.job_runs[run_index].state = ResultState::Failed;
                self.project.job_runs[run_index].progress = 1.0;
                self.project.job_runs[run_index].error = error.to_string();
                let track_id = self.project.job_runs[run_index].track_id.clone();
                if let Some(track) = self
                    .project
                    .tracks
                    .iter_mut()
                    .find(|track| track.id == track_id)
                {
                    track.result_state = ResultState::Failed;
                    track.error = error.to_string();
                    mark_dependents_stale(&mut self.project, &track_id, "");
                }
            }
        }
    }

    fn merge_job_worker_result(&mut self, result: job_worker::JobWorkerResult) {
        if result.track_id.is_empty() {
            if let Some(error) = result.error {
                self.set_error(error);
            }
            return;
        }
        let Some(current_run_index) = self
            .project
            .job_runs
            .iter()
            .position(|run| run.id == result.job_id)
        else {
            return;
        };
        let Some(current_track_index) = self
            .project
            .tracks
            .iter()
            .position(|track| track.id == result.track_id)
        else {
            return;
        };
        let current_track_state = self.project.tracks[current_track_index].result_state;
        let current_run_state = self.project.job_runs[current_run_index].state;
        let track_changed = self.project.tracks[current_track_index].dependency_hash
            != result.job_run.parameters_hash
            || !matches!(
                current_track_state,
                ResultState::Pending | ResultState::Running
            )
            || !matches!(
                current_run_state,
                ResultState::Pending | ResultState::Running
            );
        if track_changed {
            self.project.job_runs[current_run_index].state = ResultState::Stale;
            if self.project.job_runs[current_run_index].error.is_empty() {
                self.project.job_runs[current_run_index].error =
                    "track changed before async job committed".to_string();
            }
            return;
        }

        let track_id = result.track_id.clone();
        let artifact_dir = result.artifact_dir.clone();
        self.project.job_runs[current_run_index] = result.job_run;
        self.project.tracks[current_track_index] = result.track;
        self.project
            .markers
            .retain(|marker| marker.track_id != track_id);
        self.project.markers.extend(result.markers);
        for entry in result.cache_entries {
            if let Some(existing) = self
                .project
                .cache_entries
                .iter_mut()
                .find(|candidate| candidate.id == entry.id)
            {
                *existing = entry;
            } else {
                self.project.cache_entries.push(entry);
            }
        }
        let waveform_error = self
            .refresh_waveform_track_provenance(&track_id, artifact_dir.as_deref())
            .err();
        if let Some(error) = result.error.or(waveform_error) {
            self.set_error(error);
        } else {
            self.last_error = QString::default();
        }
    }

    #[cfg(test)]
    fn rerun_track_state(&mut self, track_id: &str) -> String {
        self.run_track_state(track_id)
    }

    fn runtime_params_for_track_run(&self, track_id: &str) -> Result<JsonObject, String> {
        let Some(track) = find_track(&self.project, track_id) else {
            return Ok(JsonObject::default());
        };
        let Ok(spec) = self
            .transform_registry
            .get(&track.transform_id, Some(&track.transform_version))
        else {
            return Ok(JsonObject::default());
        };
        if !spec.is_audio_input() {
            return Ok(JsonObject::default());
        }
        if let Some(path) = self.audio_artifact_path_for_track_run(track)? {
            return Ok(json_object([(
                "audio_path",
                json!(path.to_string_lossy().to_string()),
            )]));
        }
        let Some(asset) = self.source_audio_asset_for_track_id(track_id) else {
            return Err("input track has no source audio".to_string());
        };
        if asset.import_status != ImportStatus::Online {
            return Err(format!("source audio is {}", asset.import_status));
        }
        Ok(json_object([("audio_path", json!(asset.path.clone()))]))
    }

    fn audio_artifact_path_for_track_run(&self, track: &Track) -> Result<Option<PathBuf>, String> {
        let Some(artifact_dir) = self.current_artifact_dir() else {
            if track.input_track_ids.iter().any(|parent_track_id| {
                find_track(&self.project, parent_track_id).is_some_and(|parent| {
                    parent.cache_refs.iter().any(|cache_ref| {
                        self.project.cache_entries.iter().any(|entry| {
                            entry.id == *cache_ref
                                && entry.validation_status == CacheValidationStatus::Valid
                                && matches!(entry.artifact_kind.as_str(), "audio" | "stem")
                        })
                    })
                })
            }) {
                return Err(
                    "project path is required before running audio artifact transform".to_string(),
                );
            }
            return Ok(None);
        };
        let mut missing_artifact = None;
        for parent_track_id in &track.input_track_ids {
            let Some(parent) = find_track(&self.project, parent_track_id) else {
                continue;
            };
            if parent.result_state != ResultState::Complete {
                continue;
            }
            for cache_ref in &parent.cache_refs {
                let Some(entry) = self
                    .project
                    .cache_entries
                    .iter()
                    .find(|entry| entry.id == *cache_ref)
                else {
                    continue;
                };
                if entry.validation_status != CacheValidationStatus::Valid
                    || !matches!(entry.artifact_kind.as_str(), "audio" | "stem")
                {
                    continue;
                }
                if !cache_entry_path_is_safe(Path::new(&entry.path)) {
                    missing_artifact.get_or_insert_with(|| entry.path.clone());
                    continue;
                }
                let artifact_path = artifact_dir.join(&entry.path);
                if artifact_path.is_file() {
                    return Ok(Some(artifact_path));
                }
                missing_artifact.get_or_insert_with(|| artifact_path.display().to_string());
            }
        }
        if let Some(path) = missing_artifact {
            return Err(format!("audio artifact missing: {path}"));
        }
        Ok(None)
    }

    fn refresh_waveform_track_provenance(
        &mut self,
        track_id: &str,
        artifact_dir: Option<&Path>,
    ) -> Result<(), String> {
        let Some(track_index) = self
            .project
            .tracks
            .iter()
            .position(|track| track.id == track_id)
        else {
            return Ok(());
        };
        if self.project.tracks[track_index].transform_id != "waveform.summary" {
            return Ok(());
        }
        if self.project.tracks[track_index].result_state != ResultState::Complete {
            clear_waveform_provenance(&mut self.project.tracks[track_index]);
            return Ok(());
        }

        let cache_refs = self.project.tracks[track_index].cache_refs.clone();
        let Some(entry) = cache_refs.iter().find_map(|cache_ref| {
            self.project.cache_entries.iter().find(|entry| {
                entry.id == *cache_ref
                    && entry.artifact_kind == "waveform"
                    && entry.validation_status == CacheValidationStatus::Valid
            })
        }) else {
            clear_waveform_provenance(&mut self.project.tracks[track_index]);
            return Ok(());
        };
        let artifact_dir = artifact_dir
            .ok_or_else(|| "project path is required before loading waveform".to_string())?;
        let payload_path = artifact_dir.join(&entry.path);
        let payload: Value = serde_json::from_slice(
            &std::fs::read(&payload_path).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let visible = visible_samples_from_json(
            &payload,
            self.timeline_scroll_seconds,
            self.timeline_visible_seconds,
            self.timeline_pixels_per_second,
        )
        .map_err(|error| error.to_string())?;

        let track = &mut self.project.tracks[track_index];
        track
            .provenance
            .insert("waveform_payload".to_string(), payload.clone());
        track.provenance.insert(
            "waveform_samples".to_string(),
            payload
                .get("samples")
                .cloned()
                .unwrap_or_else(|| Value::Array(Vec::default())),
        );
        track.provenance.insert(
            "waveform_duration_seconds".to_string(),
            json!(payload
                .get("duration")
                .and_then(Value::as_f64)
                .unwrap_or(0.0)),
        );
        track.provenance.insert(
            "visible_waveform".to_string(),
            serde_json::to_value(visible).map_err(|error| error.to_string())?,
        );
        Ok(())
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
        if self
            .job_workers
            .iter()
            .any(|worker| worker.job_id() == job_id)
        {
            self.mark_project_mutation_dirty();
            self.refresh_selected_state();
            return;
        }
        let artifact_dir = self.current_artifact_dir();
        if let Err(error) = self
            .job_queue
            .run_next_with_artifact_dir(&mut self.project, artifact_dir.as_deref())
        {
            self.mark_project_mutation_dirty();
            self.set_error(error.to_string());
            self.refresh_view_state();
            return;
        }
        self.mark_project_mutation_dirty();
        self.refresh_view_state();
    }

    fn refresh_cache_status_state(&mut self) -> Vec<String> {
        let invalid_refs = self.validate_cache_artifact_state();
        if invalid_refs.is_empty() {
            self.last_error = QString::default();
        } else {
            self.set_error(format!("invalid cache artifacts: {}", invalid_refs.len()));
            self.mark_project_mutation_dirty();
        }
        self.refresh_view_state();
        invalid_refs
    }

    fn validate_cache_artifact_state(&mut self) -> Vec<String> {
        let project_dir = current_project_dir(&self.project_path.to_string());
        self.validate_cache_artifact_state_with_dir(project_dir.as_deref())
    }

    fn validate_cache_artifact_state_with_dir(
        &mut self,
        project_dir: Option<&Path>,
    ) -> Vec<String> {
        self.job_queue
            .refresh_cache_validity(&mut self.project, |entry| {
                cache_entry_is_valid(entry, project_dir)
            })
    }

    fn current_artifact_dir(&self) -> Option<PathBuf> {
        current_project_dir(&self.project_path.to_string())
            .or_else(|| self.demo_temp_dir.path().map(Path::to_path_buf))
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
        self.reset_job_runtime_state();
        self.clear_demo_temp_dir();
        self.project = project;
        self.project_name = QString::from(&self.project.name);
        self.project_path = QString::from(project_path.to_string_lossy().to_string());
        let project_before_load_refresh = self.project.clone();
        self.refresh_loaded_audio_assets();
        self.finalize_loaded_active_jobs();
        self.validate_cache_artifact_state();
        let load_refresh_changed_project = self.project != project_before_load_refresh;
        self.expanded_track_ids = expanded_track_ids_from_project(&self.project)
            .unwrap_or_else(|| default_expanded_track_ids(&self.project));
        self.selected_track_id = QString::from(&selected_track_id_from_project(&self.project));
        self.restore_timeline_view_state();
        self.selected_marker_ids.clear();
        self.unload_playback();
        self.reset_history_clean();
        if load_refresh_changed_project {
            self.mark_project_mutation_dirty();
        }
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
        let old_artifact_dir = self.current_artifact_dir();
        let new_artifact_dir = project_path.parent().map(Path::to_path_buf);
        let save_as_moves_artifacts = old_artifact_dir
            .as_deref()
            .zip(new_artifact_dir.as_deref())
            .is_some_and(|(old, new)| old != new);
        if save_as_moves_artifacts {
            self.validate_cache_artifact_state_with_dir(old_artifact_dir.as_deref());
            if let Err(error) = self.copy_cache_artifacts_for_save_as(
                old_artifact_dir.as_deref(),
                new_artifact_dir.as_deref(),
            ) {
                self.set_error(error);
                return false;
            }
        }
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

    fn copy_cache_artifacts_for_save_as(
        &self,
        old_artifact_dir: Option<&Path>,
        new_artifact_dir: Option<&Path>,
    ) -> Result<(), String> {
        let (Some(old_artifact_dir), Some(new_artifact_dir)) = (old_artifact_dir, new_artifact_dir)
        else {
            return Ok(());
        };
        if old_artifact_dir == new_artifact_dir {
            return Ok(());
        }
        for entry in self
            .project
            .cache_entries
            .iter()
            .filter(|entry| entry.validation_status == CacheValidationStatus::Valid)
        {
            let relative_path = Path::new(&entry.path);
            if !cache_entry_path_is_safe(relative_path) {
                continue;
            }
            let source = old_artifact_dir.join(relative_path);
            if !source.is_file() {
                continue;
            }
            let destination = new_artifact_dir.join(relative_path);
            if let Some(parent) = destination
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                fs::create_dir_all(parent).map_err(|error| {
                    format!(
                        "failed to create cache artifact directory {}: {error}",
                        parent.display()
                    )
                })?;
            }
            fs::copy(&source, &destination).map_err(|error| {
                format!(
                    "failed to copy cache artifact {} to {}: {error}",
                    source.display(),
                    destination.display()
                )
            })?;
        }
        Ok(())
    }

    fn import_audio_state(&mut self, path: &str) -> String {
        let audio_path = path_from_qml(path);
        if !audio_path.is_file() {
            self.set_error(format!("No such file: {}", audio_path.display()));
            return String::default();
        }
        let inspection = match inspect_wav_file(&audio_path) {
            Ok(inspection) => inspection,
            Err(error) => {
                self.set_error(error);
                return String::default();
            }
        };
        let asset_id = self.next_asset_id();
        let track_id = self.next_track_id();
        self.project.audio_assets.push(AudioAsset {
            id: asset_id.clone(),
            path: audio_path.to_string_lossy().to_string(),
            duration: inspection.metadata.duration,
            sample_rate: inspection.metadata.sample_rate,
            channels: inspection.metadata.channels,
            fingerprint: inspection.fingerprint,
            import_status: ImportStatus::Online,
            relink_hint: String::default(),
        });
        self.project.tracks.push(Track {
            id: track_id.clone(),
            track_type: TrackType::Source,
            name: audio_path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("Audio")
                .to_string(),
            input_track_ids: Vec::default(),
            transform_id: String::default(),
            transform_params: JsonObject::default(),
            transform_version: String::default(),
            output_schema: String::default(),
            dependency_hash: String::default(),
            result_state: ResultState::Complete,
            cache_refs: Vec::default(),
            provenance: json_object([("asset_id", json!(asset_id))]),
            error: String::default(),
        });
        self.selected_track_id = QString::from(&track_id);
        self.selected_marker_ids.clear();
        self.mark_project_mutation_dirty();
        self.last_error = QString::default();
        self.refresh_view_state();
        track_id
    }

    fn refresh_loaded_audio_assets(&mut self) -> usize {
        let project_dir = current_project_dir(&self.project_path.to_string());
        let mut affected_assets = Vec::default();
        for asset in &mut self.project.audio_assets {
            let Some(error) = audio_asset_load_error(asset) else {
                if asset.import_status != ImportStatus::Online {
                    asset.import_status = ImportStatus::Online;
                    asset.relink_hint.clear();
                    affected_assets.push((asset.id.clone(), String::default()));
                }
                continue;
            };
            if error.starts_with("input audio asset offline:") {
                if let Some(relinked_path) =
                    audio_asset_project_dir_relink_path(asset, project_dir.as_deref())
                {
                    asset.path = relinked_path.to_string_lossy().to_string();
                    asset.import_status = ImportStatus::Online;
                    asset.relink_hint.clear();
                    affected_assets.push((asset.id.clone(), String::default()));
                    continue;
                }
            }
            let status = if error.starts_with("input audio asset offline:") {
                ImportStatus::Offline
            } else {
                ImportStatus::Modified
            };
            if asset.import_status != status || asset.relink_hint.is_empty() {
                asset.import_status = status;
                asset.relink_hint = Path::new(&asset.path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_string();
            }
            affected_assets.push((asset.id.clone(), error));
        }

        let mut restored_audio_dependency_tracks = 0;
        for (asset_id, error) in &affected_assets {
            let source_track_ids = self
                .project
                .tracks
                .iter()
                .filter(|track| {
                    track.track_type == TrackType::Source
                        && track.provenance.get("asset_id").and_then(Value::as_str)
                            == Some(asset_id.as_str())
                })
                .map(|track| track.id.clone())
                .collect::<Vec<_>>();
            for track_id in source_track_ids {
                if error.is_empty() {
                    if let Some(track) = self.project.tracks.iter_mut().find(|track| {
                        track.id == track_id && is_audio_dependency_error(&track.error)
                    }) {
                        track.result_state = ResultState::Complete;
                        track.error.clear();
                    }
                } else {
                    if let Some(track) = self
                        .project
                        .tracks
                        .iter_mut()
                        .find(|track| track.id == track_id)
                    {
                        track.result_state = ResultState::Stale;
                        track.error.clone_from(error);
                    }
                    mark_dependents_stale(&mut self.project, &track_id, error);
                }
            }
        }
        if affected_assets.iter().any(|(_, error)| error.is_empty()) {
            restored_audio_dependency_tracks =
                restore_audio_dependency_dependents(&mut self.project);
        }

        affected_assets.len() + restored_audio_dependency_tracks
    }

    fn finalize_loaded_active_jobs(&mut self) -> usize {
        let mut affected_track_ids = BTreeSet::new();
        let error = "job was active when project was opened";
        let mut finalized = 0;
        for run in &mut self.project.job_runs {
            if matches!(run.state, ResultState::Pending | ResultState::Running) {
                run.state = ResultState::Stale;
                run.error = error.to_string();
                affected_track_ids.insert(run.track_id.clone());
                finalized += 1;
            }
        }

        for track_id in affected_track_ids {
            if let Some(track) = self
                .project
                .tracks
                .iter_mut()
                .find(|track| track.id == track_id)
            {
                track.result_state = ResultState::Stale;
                track.error = error.to_string();
            }
            mark_dependents_stale(&mut self.project, &track_id, error);
        }

        finalized
    }

    fn refresh_selected_state(&mut self) {
        self.reconcile_selection_with_project();
        let selected_track_id = self.selected_track_id.to_string();
        let selected_track = find_track(&self.project, &selected_track_id);
        self.selected_track_can_rerun = selected_track.is_some_and(|track| {
            track.track_type == TrackType::Generated
                && track.result_state != ResultState::Running
                && latest_active_job_id(&self.project, &selected_track_id).is_none()
                && track_inputs_are_complete(&self.project, track)
        });
        self.selected_track_has_running_job =
            latest_active_job_id(&self.project, &selected_track_id).is_some();
        self.selected_track_is_editable =
            selected_track.is_some_and(|track| track.track_type == TrackType::Editable);
        self.selected_track_can_play = self
            .source_audio_asset_for_track_id(&selected_track_id)
            .is_some_and(|asset| asset.import_status == ImportStatus::Online);
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
                return String::default();
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
        let Some(source_track) = find_track(&self.project, source_track_id) else {
            self.set_error(format!("track not found: {source_track_id}"));
            return String::default();
        };
        if source_track.result_state != ResultState::Complete {
            self.set_error(format!("source track is not complete: {source_track_id}"));
            return String::default();
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
            return String::default();
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
            transform_id: String::default(),
            transform_params: JsonObject::default(),
            transform_version: String::default(),
            output_schema: String::default(),
            dependency_hash: String::default(),
            result_state: ResultState::Complete,
            cache_refs: Vec::default(),
            provenance: json_object([
                ("source_track_id", json!(source_track_id)),
                ("source_marker_ids", json!(source_marker_ids)),
            ]),
            error: String::default(),
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
            self.refresh_visible_track_ids();
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
        let before = self.marker_edit_snapshot(&track_id, &[]);
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
                return String::default();
            }
        };
        self.selected_marker_ids = vec![marker.id.clone()];
        self.record_marker_snapshot(before, std::slice::from_ref(&marker.id));
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
        let before = self.marker_edit_snapshot(&track_id, std::slice::from_ref(&marker_id));
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
        self.record_marker_snapshot(before, std::slice::from_ref(&marker_id));
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
        let marker_ids = if self.selected_marker_ids.is_empty() {
            self.project
                .markers
                .iter()
                .filter(|marker| marker.track_id == track_id)
                .map(|marker| marker.id.clone())
                .collect()
        } else {
            self.selected_marker_ids.clone()
        };
        let before = self.marker_edit_snapshot(&track_id, &marker_ids);
        let updated = match bulk_update_editable_markers(
            &mut self.project,
            &track_id,
            &marker_ids,
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
        self.record_marker_snapshot(before, &marker_ids);
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
            let excluded_marker_ids = self.selected_marker_ids_set();
            self.project
                .markers
                .iter()
                .find(|marker| {
                    marker.track_id == track_id && marker.id == self.selected_marker_ids[0]
                })
                .map(|marker| {
                    self.snap_timeline_time_excluding(
                        marker.timestamp + delta_seconds,
                        false,
                        &excluded_marker_ids,
                    ) - marker.timestamp
                })
                .unwrap_or(delta_seconds)
        } else {
            delta_seconds
        };
        let marker_ids = self.selected_marker_ids.clone();
        let before = self.marker_edit_snapshot(&track_id, &marker_ids);
        if let Err(error) =
            move_editable_markers(&mut self.project, &track_id, &marker_ids, delta_seconds)
        {
            self.set_error(error.to_string());
            return false;
        }
        self.record_marker_snapshot(before, &marker_ids);
        self.last_error = QString::default();
        self.refresh_view_state();
        true
    }

    fn resize_marker_state(&mut self, marker_id: &str, duration: f64) -> bool {
        let track_id = self.selected_track_id.to_string();
        let marker_ids = [marker_id.to_string()];
        let before = self.marker_edit_snapshot(&track_id, &marker_ids);
        if let Err(error) =
            resize_editable_marker(&mut self.project, &track_id, marker_id, duration)
        {
            self.set_error(error.to_string());
            return false;
        }
        self.record_marker_snapshot(before, &marker_ids);
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
        let before = self.marker_edit_snapshot(&track_id, marker_ids);
        let mut deleted_ids = Vec::default();
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
            self.record_marker_snapshot(before, &deleted_ids);
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

    fn marker_edit_snapshot(&self, track_id: &str, marker_ids: &[String]) -> MarkerEditSnapshot {
        MarkerEditSnapshot {
            track_id: track_id.to_string(),
            markers: self.marker_snapshot_for_ids(track_id, marker_ids),
            dependents: self.dependent_track_snapshots(track_id),
        }
    }

    fn record_marker_snapshot(&mut self, before: MarkerEditSnapshot, marker_ids: &[String]) {
        let after = self.marker_edit_snapshot(&before.track_id, marker_ids);
        if before.markers == after.markers && before.dependents == after.dependents {
            self.sync_dirty_from_history();
            return;
        }
        self.edit_history.push(MarkerSnapshotCommand {
            track_id: before.track_id,
            before: before.markers,
            after: after.markers,
            before_dependents: before.dependents,
            after_dependents: after.dependents,
        });
        self.sync_dirty_from_history();
    }

    fn marker_snapshot_for_ids(&self, track_id: &str, marker_ids: &[String]) -> Vec<Marker> {
        let marker_ids = marker_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let mut markers = self
            .project
            .markers
            .iter()
            .filter(|marker| marker.track_id == track_id && marker_ids.contains(marker.id.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        markers.sort_by(|left, right| {
            left.timestamp
                .total_cmp(&right.timestamp)
                .then_with(|| left.id.cmp(&right.id))
        });
        markers
    }

    fn marker_snapshot_for_track(&self, track_id: &str) -> Vec<Marker> {
        let mut markers = self
            .project
            .markers
            .iter()
            .filter(|marker| marker.track_id == track_id)
            .cloned()
            .collect::<Vec<_>>();
        markers.sort_by(|left, right| {
            left.timestamp
                .total_cmp(&right.timestamp)
                .then_with(|| left.id.cmp(&right.id))
        });
        markers
    }

    fn job_runs_for_track(&self, track_id: &str) -> Vec<autolight_core::project::JobRun> {
        self.project
            .job_runs
            .iter()
            .filter(|run| run.track_id == track_id)
            .cloned()
            .collect()
    }

    fn dependent_track_snapshots(&self, track_id: &str) -> Vec<DependentTrackSnapshot> {
        let mut dependent_ids = BTreeSet::new();
        let mut pending = vec![track_id.to_string()];
        while let Some(parent_id) = pending.pop() {
            for track in &self.project.tracks {
                if track
                    .input_track_ids
                    .iter()
                    .any(|input_id| input_id == &parent_id)
                    && dependent_ids.insert(track.id.clone())
                {
                    pending.push(track.id.clone());
                }
            }
        }

        self.project
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| dependent_ids.contains(&track.id))
            .map(|(index, track)| DependentTrackSnapshot {
                track: track.clone(),
                index,
                markers: self.marker_snapshot_for_track(&track.id),
                job_runs: self.job_runs_for_track(&track.id),
            })
            .collect()
    }

    fn reset_history_clean(&mut self) {
        self.edit_history.clear();
        self.non_history_dirty = false;
        self.sync_dirty_from_history();
    }

    fn mark_clean(&mut self) {
        self.edit_history.mark_clean();
        self.non_history_dirty = false;
        self.sync_dirty_from_history();
    }

    fn mark_non_history_dirty(&mut self) {
        self.non_history_dirty = true;
        self.sync_dirty_from_history();
    }

    fn mark_project_mutation_dirty(&mut self) {
        self.non_history_dirty = true;
        self.edit_history.clear();
        self.edit_history.mark_clean();
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

    fn viewport_property_values(&self) -> ViewportPropertyValues {
        ViewportPropertyValues {
            timeline_duration_seconds: self.timeline_duration_seconds,
            timeline_pixels_per_second: self.timeline_pixels_per_second,
            timeline_scroll_seconds: self.timeline_scroll_seconds,
            timeline_visible_seconds: self.timeline_visible_seconds,
        }
    }

    fn selection_property_values(&self) -> SelectionPropertyValues {
        SelectionPropertyValues {
            selected_track_id: self.selected_track_id.clone(),
            selected_track_can_rerun: self.selected_track_can_rerun,
            selected_track_has_running_job: self.selected_track_has_running_job,
            selected_track_is_editable: self.selected_track_is_editable,
            selected_track_can_play: self.selected_track_can_play,
            selected_marker_ids_json: self.selected_marker_ids_json.clone(),
            selected_track_markers_json: self.selected_track_markers_json.clone(),
            marker_color_options_json: self.marker_color_options_json.clone(),
        }
    }

    fn playback_property_values(&self) -> PlaybackPropertyValues {
        PlaybackPropertyValues {
            playback_source_path: self.playback_source_path.clone(),
            playback_position_seconds: self.playback_position_seconds,
            playback_duration_seconds: self.playback_duration_seconds,
            playback_is_playing: self.playback_is_playing,
            playback_last_error: self.playback_last_error.clone(),
            playback_volume: self.playback_volume,
        }
    }
}

pub(crate) fn runnable_transform_ids() -> &'static [&'static str] {
    &["markers.fixed_interval", "waveform.summary"]
}

pub(crate) fn is_runnable_transform_id(transform_id: &str) -> bool {
    runnable_transform_ids().contains(&transform_id)
}

struct ViewportPropertyValues {
    timeline_duration_seconds: f64,
    timeline_pixels_per_second: f64,
    timeline_scroll_seconds: f64,
    timeline_visible_seconds: f64,
}

struct SelectionPropertyValues {
    selected_track_id: QString,
    selected_track_can_rerun: bool,
    selected_track_has_running_job: bool,
    selected_track_is_editable: bool,
    selected_track_can_play: bool,
    selected_marker_ids_json: QString,
    selected_track_markers_json: QString,
    marker_color_options_json: QString,
}

struct PlaybackPropertyValues {
    playback_source_path: QString,
    playback_position_seconds: f64,
    playback_duration_seconds: f64,
    playback_is_playing: bool,
    playback_last_error: QString,
    playback_volume: f64,
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
        #[cxx_name = "pollJobs"]
        fn poll_jobs(self: Pin<&mut Self>) -> i32;

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
        #[cxx_name = "setTimelineVisibleTrackRange"]
        fn set_timeline_visible_track_range(self: Pin<&mut Self>, first_row: i32, row_count: i32);

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
            let job_id = state.submit_track_state(&track_id.to_string());
            (state.qproperty_values(), job_id)
        };
        self.apply_values(values);
        QString::from(&job_id)
    }

    pub fn rerun_track(mut self: Pin<&mut Self>, track_id: QString) -> QString {
        let (values, job_id) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let job_id = state.submit_track_state(&track_id.to_string());
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

    pub fn poll_jobs(mut self: Pin<&mut Self>) -> i32 {
        let (values, changed) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let changed = state.poll_job_workers_state();
            (state.qproperty_values(), changed)
        };
        self.apply_values(values);
        changed
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
        let (playback_values, selection_values) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            state.pause_playback_state();
            (
                state.playback_property_values(),
                state.selection_property_values(),
            )
        };
        self.as_mut().apply_playback_values(playback_values);
        self.apply_selection_values(selection_values);
    }

    pub fn stop_playback(mut self: Pin<&mut Self>) {
        let (playback_values, viewport_values, selection_values) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            state.stop_playback_state();
            (
                state.playback_property_values(),
                state.viewport_property_values(),
                state.selection_property_values(),
            )
        };
        self.as_mut().apply_playback_values(playback_values);
        self.as_mut().apply_viewport_values(viewport_values);
        self.apply_selection_values(selection_values);
    }

    pub fn seek_playback(mut self: Pin<&mut Self>, seconds: f64) {
        let (playback_values, viewport_values, selection_values) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            state.seek_playback_state(seconds);
            (
                state.playback_property_values(),
                state.viewport_property_values(),
                state.selection_property_values(),
            )
        };
        self.as_mut().apply_playback_values(playback_values);
        self.as_mut().apply_viewport_values(viewport_values);
        self.apply_selection_values(selection_values);
    }

    pub fn nudge_playback(mut self: Pin<&mut Self>, delta_seconds: f64) {
        let (playback_values, viewport_values, selection_values) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            state.nudge_playback_state(delta_seconds);
            (
                state.playback_property_values(),
                state.viewport_property_values(),
                state.selection_property_values(),
            )
        };
        self.as_mut().apply_playback_values(playback_values);
        self.as_mut().apply_viewport_values(viewport_values);
        self.apply_selection_values(selection_values);
    }

    pub fn set_playback_volume_invokable(mut self: Pin<&mut Self>, value: f64) {
        let playback_values = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            state.set_playback_volume_state(value);
            state.playback_property_values()
        };
        self.apply_playback_values(playback_values);
    }

    pub fn set_timeline_zoom(mut self: Pin<&mut Self>, pixels_per_second: f64) {
        let (viewport_values, selection_values) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            state.set_timeline_zoom_state(pixels_per_second);
            (
                state.viewport_property_values(),
                state.selection_property_values(),
            )
        };
        self.as_mut().apply_viewport_values(viewport_values);
        self.apply_selection_values(selection_values);
    }

    pub fn set_timeline_scroll_seconds_invokable(mut self: Pin<&mut Self>, seconds: f64) {
        let (viewport_values, selection_values) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            state.set_timeline_scroll_seconds_state(seconds);
            (
                state.viewport_property_values(),
                state.selection_property_values(),
            )
        };
        self.as_mut().apply_viewport_values(viewport_values);
        self.apply_selection_values(selection_values);
    }

    pub fn set_timeline_visible_seconds_invokable(mut self: Pin<&mut Self>, seconds: f64) {
        let (viewport_values, selection_values) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            state.set_timeline_visible_seconds_state(seconds);
            (
                state.viewport_property_values(),
                state.selection_property_values(),
            )
        };
        self.as_mut().apply_viewport_values(viewport_values);
        self.apply_selection_values(selection_values);
    }

    pub fn set_timeline_visible_track_range(
        mut self: Pin<&mut Self>,
        first_row: i32,
        row_count: i32,
    ) {
        let viewport_values = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            state.set_timeline_visible_track_range_state(first_row, row_count);
            state.viewport_property_values()
        };
        self.apply_viewport_values(viewport_values);
    }

    pub fn snap_timeline_time(mut self: Pin<&mut Self>, seconds: f64, bypass_snap: bool) -> f64 {
        let (viewport_values, snapped) = {
            let mut rust = self.as_mut().rust_mut();
            let state = rust.as_mut().get_mut();
            let snapped = state.snap_timeline_time_state(seconds, bypass_snap);
            (state.viewport_property_values(), snapped)
        };
        self.apply_viewport_values(viewport_values);
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

    fn apply_viewport_values(mut self: Pin<&mut Self>, values: ViewportPropertyValues) {
        self.as_mut()
            .set_timeline_duration_seconds(values.timeline_duration_seconds);
        self.as_mut()
            .set_timeline_pixels_per_second(values.timeline_pixels_per_second);
        self.as_mut()
            .set_timeline_scroll_seconds(values.timeline_scroll_seconds);
        self.as_mut()
            .set_timeline_visible_seconds(values.timeline_visible_seconds);
    }

    fn apply_selection_values(mut self: Pin<&mut Self>, values: SelectionPropertyValues) {
        self.as_mut()
            .set_selected_track_id(values.selected_track_id);
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
    }

    fn apply_playback_values(mut self: Pin<&mut Self>, values: PlaybackPropertyValues) {
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
