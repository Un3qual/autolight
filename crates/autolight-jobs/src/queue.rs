use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use autolight_core::cache::{
    cache_entry_for_bytes, invalid_cache_refs, mark_invalid_cache_refs_stale,
    track_dependency_hash, track_dependency_inputs, upsert_cache_entry, CacheError,
};
use autolight_core::graph::{find_track, mark_dependents_stale};
use autolight_core::project::{
    CacheEntry, JobRun, JsonObject, Marker, ProjectDocument, ResultState, TrackType,
};
use autolight_core::transforms::{TransformError, TransformRegistry, TransformSpec};
use thiserror::Error;

const RUNTIME_ONLY_TRANSFORM_PARAM_KEYS: &[&str] = &["audio_path"];

type RunnerFn =
    dyn FnMut(&mut TransformContext, &JsonObject) -> Result<TransformResult, TransformRunError>;

#[derive(Debug, Error)]
pub enum JobQueueError {
    #[error("track not found: {0}")]
    TrackNotFound(String),
    #[error("jobs can only run generated tracks")]
    TrackNotGenerated,
    #[error("track has no transform id")]
    MissingTransform,
    #[error("job already pending or running for track: {0}")]
    DuplicateRunningTrack(String),
    #[error("job not found: {0}")]
    JobNotFound(String),
    #[error("runtime transform params cannot change cached identity")]
    RuntimeParamsChangeIdentity,
    #[error("marker timestamp must be finite and non-negative")]
    NonFiniteMarkerTimestamp,
    #[error("marker duration must be finite and non-negative")]
    InvalidMarkerDuration,
    #[error("input track is not complete: {0}")]
    InputTrackNotComplete(String),
    #[error(transparent)]
    Transform(#[from] TransformError),
    #[error(transparent)]
    Cache(#[from] CacheError),
    #[error("project path is required before caching artifacts")]
    MissingArtifactDirectory,
    #[error("unsafe cache artifact path: {0}")]
    UnsafeArtifactPath(String),
    #[error("failed to write cache artifact {path}: {source}")]
    ArtifactWrite { path: PathBuf, source: io::Error },
}

#[derive(Debug, Error)]
pub enum TransformRunError {
    #[error("cancelled")]
    Cancelled,
    #[error("{0}")]
    Failed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransformContext {
    job_id: String,
    track_id: String,
    progress: f64,
    cancel_requested: bool,
}

impl TransformContext {
    fn new(job_id: String, track_id: String, cancel_requested: bool) -> Self {
        Self {
            job_id,
            track_id,
            progress: 0.0,
            cancel_requested,
        }
    }

    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    pub fn track_id(&self) -> &str {
        &self.track_id
    }

    pub fn progress(&self) -> f64 {
        self.progress
    }

    pub fn report_progress(&mut self, progress: f64) {
        if progress.is_finite() {
            self.progress = progress.clamp(0.0, 1.0);
        }
    }

    pub fn cancel_requested(&self) -> bool {
        self.cancel_requested
    }

    pub fn request_cancel(&mut self) {
        self.cancel_requested = true;
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct TransformResult {
    pub markers: Vec<ProducedMarker>,
    pub artifacts: Vec<ProducedArtifact>,
}

impl TransformResult {
    pub fn markers(markers: Vec<ProducedMarker>) -> Self {
        Self {
            markers,
            artifacts: Vec::new(),
        }
    }

    pub fn artifact(artifact_kind: impl Into<String>, payload: impl Into<Vec<u8>>) -> Self {
        Self {
            markers: Vec::new(),
            artifacts: vec![ProducedArtifact {
                artifact_kind: artifact_kind.into(),
                payload: payload.into(),
            }],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProducedMarker {
    pub timestamp: f64,
    pub duration: Option<f64>,
    pub label: String,
    pub category: String,
    pub confidence: Option<f64>,
    pub tags: Vec<String>,
    pub metadata: JsonObject,
}

impl ProducedMarker {
    pub fn new(timestamp: f64, label: impl Into<String>) -> Self {
        Self {
            timestamp,
            duration: None,
            label: label.into(),
            category: String::new(),
            confidence: None,
            tags: Vec::new(),
            metadata: JsonObject::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProducedArtifact {
    pub artifact_kind: String,
    pub payload: Vec<u8>,
}

#[derive(Default)]
pub struct JobRegistry {
    specs: TransformRegistry,
    runners: BTreeMap<(String, String), Box<RunnerFn>>,
}

impl JobRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<F>(&mut self, spec: TransformSpec, runner: F) -> Result<(), TransformError>
    where
        F: FnMut(&mut TransformContext, &JsonObject) -> Result<TransformResult, TransformRunError>
            + 'static,
    {
        let key = (spec.id.clone(), spec.version.clone());
        self.specs.register(spec)?;
        self.runners.insert(key, Box::new(runner));
        Ok(())
    }

    fn spec(&self, transform_id: &str, version: &str) -> Result<&TransformSpec, TransformError> {
        self.specs.get(transform_id, Some(version))
    }

    fn runner_mut(
        &mut self,
        transform_id: &str,
        version: &str,
    ) -> Result<&mut Box<RunnerFn>, TransformError> {
        self.spec(transform_id, version)?;
        self.runners
            .get_mut(&(transform_id.to_string(), version.to_string()))
            .ok_or_else(|| TransformError::UnknownTransform(transform_id.to_string()))
    }
}

pub struct LocalJobQueue {
    registry: JobRegistry,
    pending_job_ids: VecDeque<String>,
    cancel_requests: BTreeSet<String>,
    next_job_number: u64,
    now: Box<dyn FnMut() -> String>,
}

impl LocalJobQueue {
    pub fn new(registry: JobRegistry) -> Self {
        Self {
            registry,
            pending_job_ids: VecDeque::new(),
            cancel_requests: BTreeSet::new(),
            next_job_number: 1,
            now: Box::new(default_timestamp),
        }
    }

    pub fn with_clock(registry: JobRegistry, now: impl FnMut() -> String + 'static) -> Self {
        Self {
            registry,
            pending_job_ids: VecDeque::new(),
            cancel_requests: BTreeSet::new(),
            next_job_number: 1,
            now: Box::new(now),
        }
    }

    pub fn submit(
        &mut self,
        project: &mut ProjectDocument,
        track_id: &str,
    ) -> Result<String, JobQueueError> {
        self.submit_with_runtime_params(project, track_id, JsonObject::new())
    }

    pub fn submit_with_runtime_params(
        &mut self,
        project: &mut ProjectDocument,
        track_id: &str,
        runtime_params: JsonObject,
    ) -> Result<String, JobQueueError> {
        self.seed_next_job_number(project);
        if has_active_job(project, track_id) {
            return Err(JobQueueError::DuplicateRunningTrack(track_id.to_string()));
        }

        let snapshot = track_snapshot_for_submit(project, track_id)?;
        if snapshot.transform_id.is_empty() {
            return Err(JobQueueError::MissingTransform);
        }
        self.registry
            .spec(&snapshot.transform_id, &snapshot.transform_version)?;
        let run_params = merge_runtime_params(&snapshot.transform_params, &runtime_params)?;
        let identity_params = identity_params(&run_params);
        let dependency_hash = dependency_hash_for_track(
            project,
            &snapshot.input_track_ids,
            &snapshot.transform_id,
            &snapshot.transform_version,
            &identity_params,
        )?;
        let job_id = self.next_job_id();

        let track = project
            .tracks
            .iter_mut()
            .find(|track| track.id == track_id)
            .ok_or_else(|| JobQueueError::TrackNotFound(track_id.to_string()))?;
        track.dependency_hash = dependency_hash.clone();
        track.result_state = ResultState::Pending;
        track.error.clear();

        project.job_runs.push(JobRun {
            id: job_id.clone(),
            track_id: track_id.to_string(),
            transform_id: snapshot.transform_id,
            parameters_hash: dependency_hash,
            parameters: run_params,
            state: ResultState::Pending,
            progress: 0.0,
            started_at: String::new(),
            completed_at: String::new(),
            error: String::new(),
            produced_cache_refs: Vec::new(),
        });
        self.pending_job_ids.push_back(job_id.clone());
        Ok(job_id)
    }

    pub fn rerun(
        &mut self,
        project: &mut ProjectDocument,
        track_id: &str,
    ) -> Result<String, JobQueueError> {
        self.submit(project, track_id)
    }

    pub fn cancel(&mut self, job_id: &str) -> Result<(), JobQueueError> {
        if self
            .pending_job_ids
            .iter()
            .any(|pending_id| pending_id == job_id)
        {
            self.cancel_requests.insert(job_id.to_string());
            return Ok(());
        }
        Err(JobQueueError::JobNotFound(job_id.to_string()))
    }

    pub fn run_next(
        &mut self,
        project: &mut ProjectDocument,
    ) -> Result<Option<String>, JobQueueError> {
        self.run_next_with_artifact_dir(project, None)
    }

    pub fn run_next_with_artifact_dir(
        &mut self,
        project: &mut ProjectDocument,
        artifact_dir: Option<&Path>,
    ) -> Result<Option<String>, JobQueueError> {
        let Some(job_id) = self.pending_job_ids.pop_front() else {
            return Ok(None);
        };
        if pending_track_is_stale(project, &job_id)? {
            self.cancel_requests.remove(&job_id);
            self.complete_stale_run(project, &job_id, "track changed before job started")?;
            return Ok(Some(job_id));
        }
        self.start_job(project, &job_id)?;
        if self.cancel_requests.remove(&job_id) {
            self.complete_cancelled(project, &job_id, "cancelled")?;
            return Ok(Some(job_id));
        }

        let run_snapshot = run_snapshot(project, &job_id)?;
        let mut context =
            TransformContext::new(job_id.clone(), run_snapshot.track_id.clone(), false);
        let runner = self
            .registry
            .runner_mut(&run_snapshot.transform_id, &run_snapshot.transform_version)?;
        let result = runner(&mut context, &run_snapshot.params);

        if context.cancel_requested() {
            self.complete_cancelled(project, &job_id, "cancelled")?;
            return Ok(Some(job_id));
        }

        match result {
            Ok(result) => {
                if let Err(error) = self.complete_success(
                    project,
                    &job_id,
                    context.progress(),
                    result,
                    artifact_dir,
                ) {
                    let message = error.to_string();
                    self.complete_failed(project, &job_id, &message)?;
                    return Err(error);
                }
            }
            Err(TransformRunError::Cancelled) => {
                self.complete_cancelled(project, &job_id, "cancelled")?;
            }
            Err(TransformRunError::Failed(error)) => {
                self.complete_failed(project, &job_id, &error)?;
            }
        }
        Ok(Some(job_id))
    }

    pub fn refresh_cache_validity(
        &mut self,
        project: &mut ProjectDocument,
        is_entry_valid: impl FnMut(&CacheEntry) -> bool,
    ) -> Vec<String> {
        let invalid = invalid_cache_refs(project, is_entry_valid);
        let invalid_ids = invalid.iter().collect::<BTreeSet<_>>();
        for entry in &mut project.cache_entries {
            if invalid_ids.contains(&entry.id) {
                entry.validation_status = "invalid".to_string();
            }
        }
        mark_invalid_cache_refs_stale(project, &invalid);
        invalid
    }

    fn start_job(
        &mut self,
        project: &mut ProjectDocument,
        job_id: &str,
    ) -> Result<(), JobQueueError> {
        let run_index = job_run_index(project, job_id)?;
        let track_id = project.job_runs[run_index].track_id.clone();
        let started_at = self.timestamp();
        project.job_runs[run_index].state = ResultState::Running;
        project.job_runs[run_index].started_at = started_at;
        project.job_runs[run_index].progress = 0.0;

        let track = project
            .tracks
            .iter_mut()
            .find(|track| track.id == track_id)
            .ok_or_else(|| JobQueueError::TrackNotFound(track_id.clone()))?;
        track.result_state = ResultState::Running;
        track.error.clear();
        Ok(())
    }

    fn complete_success(
        &mut self,
        project: &mut ProjectDocument,
        job_id: &str,
        progress: f64,
        result: TransformResult,
        artifact_dir: Option<&Path>,
    ) -> Result<(), JobQueueError> {
        let run = run_snapshot(project, job_id)?;
        if track_changed_since_submit(project, &run.track_id, &run.parameters_hash)? {
            self.complete_stale_run(project, job_id, "track changed before job committed")?;
            return Ok(());
        }

        let markers = build_markers(job_id, &run.track_id, &run.transform_id, result.markers)?;
        let cache_artifacts = result
            .artifacts
            .into_iter()
            .map(|artifact| {
                let entry = cache_entry_for_bytes(
                    &artifact.artifact_kind,
                    &run.parameters_hash,
                    &artifact.payload,
                    &run.transform_version,
                    self.timestamp(),
                )?;
                Ok::<(CacheEntry, Vec<u8>), CacheError>((entry, artifact.payload))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let produced_cache_refs = cache_artifacts
            .iter()
            .map(|(entry, _)| entry.id.clone())
            .collect::<Vec<_>>();

        if !cache_artifacts.is_empty() {
            let artifact_dir = artifact_dir.ok_or(JobQueueError::MissingArtifactDirectory)?;
            for (entry, payload) in &cache_artifacts {
                write_cache_artifact_payload(artifact_dir, entry, payload)?;
            }
        }

        for (entry, _) in cache_artifacts {
            upsert_cache_entry(project, entry);
        }
        project
            .markers
            .retain(|marker| marker.track_id != run.track_id);
        project.markers.extend(markers);

        let track = project
            .tracks
            .iter_mut()
            .find(|track| track.id == run.track_id)
            .ok_or_else(|| JobQueueError::TrackNotFound(run.track_id.clone()))?;
        track.cache_refs = produced_cache_refs.clone();
        track.result_state = ResultState::Complete;
        track.error.clear();

        let completed_at = self.timestamp();
        let run_index = job_run_index(project, job_id)?;
        project.job_runs[run_index].state = ResultState::Complete;
        project.job_runs[run_index].progress = progress.max(1.0);
        project.job_runs[run_index].completed_at = completed_at;
        project.job_runs[run_index].error.clear();
        project.job_runs[run_index].produced_cache_refs = produced_cache_refs;

        mark_dependents_stale(project, &run.track_id, "");
        Ok(())
    }

    fn complete_failed(
        &mut self,
        project: &mut ProjectDocument,
        job_id: &str,
        error: &str,
    ) -> Result<(), JobQueueError> {
        let run = run_snapshot(project, job_id)?;
        if track_changed_since_submit(project, &run.track_id, &run.parameters_hash)? {
            self.complete_stale_run(project, job_id, "track changed before job failed")?;
            return Ok(());
        }
        self.complete_terminal(project, job_id, ResultState::Failed, error)
    }

    fn complete_cancelled(
        &mut self,
        project: &mut ProjectDocument,
        job_id: &str,
        error: &str,
    ) -> Result<(), JobQueueError> {
        self.complete_terminal(project, job_id, ResultState::Cancelled, error)
    }

    fn complete_stale_run(
        &mut self,
        project: &mut ProjectDocument,
        job_id: &str,
        error: &str,
    ) -> Result<(), JobQueueError> {
        let completed_at = self.timestamp();
        let run_index = job_run_index(project, job_id)?;
        project.job_runs[run_index].state = ResultState::Stale;
        project.job_runs[run_index].completed_at = completed_at;
        project.job_runs[run_index].error = error.to_string();
        Ok(())
    }

    fn complete_terminal(
        &mut self,
        project: &mut ProjectDocument,
        job_id: &str,
        state: ResultState,
        error: &str,
    ) -> Result<(), JobQueueError> {
        let run_index = job_run_index(project, job_id)?;
        let track_id = project.job_runs[run_index].track_id.clone();
        let completed_at = self.timestamp();
        project.job_runs[run_index].state = state;
        project.job_runs[run_index].completed_at = completed_at;
        project.job_runs[run_index].error = error.to_string();

        let track = project
            .tracks
            .iter_mut()
            .find(|track| track.id == track_id)
            .ok_or_else(|| JobQueueError::TrackNotFound(track_id.clone()))?;
        track.result_state = state;
        track.error = error.to_string();
        mark_dependents_stale(project, &track_id, "");
        Ok(())
    }

    fn next_job_id(&mut self) -> String {
        let job_id = format!("job_{:04}", self.next_job_number);
        self.next_job_number += 1;
        job_id
    }

    fn seed_next_job_number(&mut self, project: &ProjectDocument) {
        let next_from_project = project
            .job_runs
            .iter()
            .filter_map(|run| run.id.strip_prefix("job_"))
            .filter_map(|suffix| suffix.parse::<u64>().ok())
            .max()
            .map(|value| value + 1)
            .unwrap_or(1);
        self.next_job_number = self.next_job_number.max(next_from_project);
    }

    fn timestamp(&mut self) -> String {
        (self.now)()
    }
}

#[derive(Clone)]
struct SubmitTrackSnapshot {
    input_track_ids: Vec<String>,
    transform_id: String,
    transform_version: String,
    transform_params: JsonObject,
}

#[derive(Clone)]
struct RunSnapshot {
    track_id: String,
    transform_id: String,
    transform_version: String,
    parameters_hash: String,
    params: JsonObject,
}

fn track_snapshot_for_submit(
    project: &ProjectDocument,
    track_id: &str,
) -> Result<SubmitTrackSnapshot, JobQueueError> {
    let track = find_track(project, track_id)
        .ok_or_else(|| JobQueueError::TrackNotFound(track_id.to_string()))?;
    if track.track_type != TrackType::Generated {
        return Err(JobQueueError::TrackNotGenerated);
    }
    Ok(SubmitTrackSnapshot {
        input_track_ids: track.input_track_ids.clone(),
        transform_id: track.transform_id.clone(),
        transform_version: track.transform_version.clone(),
        transform_params: track.transform_params.clone(),
    })
}

fn run_snapshot(project: &ProjectDocument, job_id: &str) -> Result<RunSnapshot, JobQueueError> {
    let run_index = project
        .job_runs
        .iter()
        .position(|run| run.id == job_id)
        .ok_or_else(|| JobQueueError::JobNotFound(job_id.to_string()))?;
    let run = &project.job_runs[run_index];
    let track = find_track(project, &run.track_id)
        .ok_or_else(|| JobQueueError::TrackNotFound(run.track_id.clone()))?;
    Ok(RunSnapshot {
        track_id: run.track_id.clone(),
        transform_id: run.transform_id.clone(),
        transform_version: track.transform_version.clone(),
        parameters_hash: run.parameters_hash.clone(),
        params: if run.parameters.is_empty() {
            track.transform_params.clone()
        } else {
            run.parameters.clone()
        },
    })
}

fn dependency_hash_for_track(
    project: &ProjectDocument,
    input_track_ids: &[String],
    transform_id: &str,
    transform_version: &str,
    identity_params: &JsonObject,
) -> Result<String, JobQueueError> {
    let mut input_cache_refs = Vec::new();
    for input_track_id in input_track_ids {
        let input_track = find_track(project, input_track_id)
            .ok_or_else(|| JobQueueError::TrackNotFound(input_track_id.clone()))?;
        if input_track.result_state != ResultState::Complete {
            return Err(JobQueueError::InputTrackNotComplete(input_track_id.clone()));
        }
        input_cache_refs.extend(track_dependency_inputs(project, input_track)?);
    }
    Ok(track_dependency_hash(
        &input_cache_refs,
        transform_id,
        transform_version,
        identity_params,
    )?)
}

fn merge_runtime_params(
    saved_params: &JsonObject,
    runtime_params: &JsonObject,
) -> Result<JsonObject, JobQueueError> {
    for (key, runtime_value) in runtime_params {
        if is_runtime_only_param(key) {
            continue;
        }
        if saved_params.get(key) != Some(runtime_value) {
            return Err(JobQueueError::RuntimeParamsChangeIdentity);
        }
    }

    let mut merged = saved_params.clone();
    for (key, value) in runtime_params {
        merged.insert(key.clone(), value.clone());
    }
    Ok(merged)
}

fn identity_params(params: &JsonObject) -> JsonObject {
    params
        .iter()
        .filter(|(key, _)| !is_runtime_only_param(key))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn is_runtime_only_param(key: &str) -> bool {
    RUNTIME_ONLY_TRANSFORM_PARAM_KEYS.contains(&key)
}

fn has_active_job(project: &ProjectDocument, track_id: &str) -> bool {
    project.job_runs.iter().any(|run| {
        run.track_id == track_id && matches!(run.state, ResultState::Pending | ResultState::Running)
    })
}

fn build_markers(
    job_id: &str,
    track_id: &str,
    transform_id: &str,
    produced_markers: Vec<ProducedMarker>,
) -> Result<Vec<Marker>, JobQueueError> {
    produced_markers
        .into_iter()
        .enumerate()
        .map(|(index, marker)| {
            if !marker.timestamp.is_finite() || marker.timestamp < 0.0 {
                return Err(JobQueueError::NonFiniteMarkerTimestamp);
            }
            if marker
                .duration
                .is_some_and(|duration| !duration.is_finite() || duration < 0.0)
            {
                return Err(JobQueueError::InvalidMarkerDuration);
            }
            Ok(Marker {
                id: format!("marker_{job_id}_{index:04}"),
                track_id: track_id.to_string(),
                timestamp: marker.timestamp,
                duration: marker.duration,
                label: marker.label,
                category: marker.category,
                confidence: marker.confidence,
                tags: marker.tags,
                source_transform: transform_id.to_string(),
                source_marker_ids: Vec::new(),
                metadata: marker.metadata,
            })
        })
        .collect()
}

fn track_changed_since_submit(
    project: &ProjectDocument,
    track_id: &str,
    parameters_hash: &str,
) -> Result<bool, JobQueueError> {
    let track = find_track(project, track_id)
        .ok_or_else(|| JobQueueError::TrackNotFound(track_id.to_string()))?;
    Ok(track.dependency_hash != parameters_hash || track.result_state == ResultState::Stale)
}

fn pending_track_is_stale(project: &ProjectDocument, job_id: &str) -> Result<bool, JobQueueError> {
    let run = run_snapshot(project, job_id)?;
    let track = find_track(project, &run.track_id)
        .ok_or_else(|| JobQueueError::TrackNotFound(run.track_id.clone()))?;
    Ok(track.result_state == ResultState::Stale)
}

fn job_run_index(project: &ProjectDocument, job_id: &str) -> Result<usize, JobQueueError> {
    project
        .job_runs
        .iter()
        .position(|run| run.id == job_id)
        .ok_or_else(|| JobQueueError::JobNotFound(job_id.to_string()))
}

fn default_timestamp() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:09}Z", duration.as_secs(), duration.subsec_nanos())
}

fn write_cache_artifact_payload(
    artifact_dir: &Path,
    entry: &CacheEntry,
    payload: &[u8],
) -> Result<(), JobQueueError> {
    let relative_path = Path::new(&entry.path);
    if !cache_artifact_path_is_safe(relative_path) {
        return Err(JobQueueError::UnsafeArtifactPath(entry.path.clone()));
    }

    let path = artifact_dir.join(relative_path);
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| JobQueueError::ArtifactWrite {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let tmp_path = cache_artifact_temp_path(&path);
    let write_result = (|| -> Result<(), JobQueueError> {
        let mut file =
            fs::File::create(&tmp_path).map_err(|source| JobQueueError::ArtifactWrite {
                path: tmp_path.clone(),
                source,
            })?;
        file.write_all(payload)
            .map_err(|source| JobQueueError::ArtifactWrite {
                path: tmp_path.clone(),
                source,
            })?;
        file.sync_all()
            .map_err(|source| JobQueueError::ArtifactWrite {
                path: tmp_path.clone(),
                source,
            })?;
        drop(file);
        fs::rename(&tmp_path, &path).map_err(|source| JobQueueError::ArtifactWrite {
            path: path.clone(),
            source,
        })
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }
    write_result
}

fn cache_artifact_path_is_safe(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
}

fn cache_artifact_temp_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("artifact.bin");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    path.with_file_name(format!(".{file_name}.tmp-{}-{nonce}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use autolight_core::cache::track_dependency_hash;
    use autolight_core::graph::mark_dependents_stale;
    use autolight_core::project::{AudioAsset, CacheEntry, Track};
    use serde_json::{json, Value};

    use super::{JobRegistry, LocalJobQueue, ProducedMarker, TransformResult, TransformRunError};
    use crate::queue::ProducedArtifact;
    use autolight_core::project::{JsonObject, Marker, ProjectDocument, ResultState, TrackType};
    use autolight_core::transforms::TransformSpec;

    #[test]
    fn jobs_submit_then_run_completes_markers_and_artifact_cache_refs() {
        let mut registry = JobRegistry::default();
        registry
            .register(
                test_spec("test.markers_and_artifact"),
                |context, _params| {
                    context.report_progress(0.5);
                    Ok(TransformResult {
                        markers: vec![
                            ProducedMarker::new(0.0, "Beat"),
                            ProducedMarker::new(0.5, "Beat"),
                        ],
                        artifacts: vec![ProducedArtifact {
                            artifact_kind: "stem".to_string(),
                            payload: b"cached stem".to_vec(),
                        }],
                    })
                },
            )
            .unwrap();
        let mut project = project_with_generated_track("test.markers_and_artifact");
        let mut queue = LocalJobQueue::with_clock(registry, deterministic_clock());

        let job_id = queue.submit(&mut project, "track_generated").unwrap();
        assert_eq!(
            track_state(&project, "track_generated"),
            ResultState::Pending
        );
        assert_eq!(job_state(&project, &job_id), ResultState::Pending);
        let artifact_dir = test_dir("artifact-cache");

        assert_eq!(
            queue
                .run_next_with_artifact_dir(&mut project, Some(&artifact_dir))
                .unwrap(),
            Some(job_id.clone())
        );

        let track = track_by_id(&project, "track_generated");
        let run = run_by_id(&project, &job_id);
        assert_eq!(track.result_state, ResultState::Complete);
        assert_eq!(run.state, ResultState::Complete);
        assert_eq!(run.progress, 1.0);
        assert_eq!(project.markers.len(), 2);
        assert_eq!(project.cache_entries.len(), 1);
        assert_eq!(project.cache_entries[0].artifact_kind, "stem");
        assert_eq!(
            std::fs::read(artifact_dir.join(&project.cache_entries[0].path)).unwrap(),
            b"cached stem"
        );
        assert_eq!(track.cache_refs, vec![project.cache_entries[0].id.clone()]);
        assert_eq!(run.produced_cache_refs, track.cache_refs);
    }

    #[test]
    fn jobs_artifact_output_requires_cache_directory() {
        let mut registry = JobRegistry::default();
        registry
            .register(test_spec("test.artifact"), |_context, _params| {
                Ok(TransformResult::artifact("stem", b"cached stem"))
            })
            .unwrap();
        let mut project = project_with_generated_track("test.artifact");
        let mut queue = LocalJobQueue::with_clock(registry, deterministic_clock());

        let job_id = queue.submit(&mut project, "track_generated").unwrap();
        let error = queue.run_next(&mut project).unwrap_err();

        assert!(error
            .to_string()
            .contains("project path is required before caching artifacts"));
        assert_eq!(job_state(&project, &job_id), ResultState::Failed);
        assert_eq!(
            track_state(&project, "track_generated"),
            ResultState::Failed
        );
        assert!(project.cache_entries.is_empty());
        assert!(track_by_id(&project, "track_generated")
            .cache_refs
            .is_empty());
    }

    #[test]
    fn jobs_preserve_stale_pending_track_before_runner_starts() {
        let mut project = project_with_generated_track("test.noop");
        let mut queue =
            LocalJobQueue::with_clock(registry_with_noop("test.noop"), deterministic_clock());

        let job_id = queue.submit(&mut project, "track_generated").unwrap();
        mark_dependents_stale(&mut project, "track_source", "input changed");

        assert_eq!(queue.run_next(&mut project).unwrap(), Some(job_id.clone()));

        assert_eq!(job_state(&project, &job_id), ResultState::Stale);
        assert_eq!(track_state(&project, "track_generated"), ResultState::Stale);
        assert!(run_by_id(&project, &job_id)
            .error
            .contains("track changed before job started"));
        assert!(project.markers.is_empty());
        assert!(track_by_id(&project, "track_generated")
            .cache_refs
            .is_empty());
    }

    #[test]
    fn jobs_reject_duplicate_pending_submit_for_track() {
        let mut project = project_with_generated_track("test.noop");
        let mut queue = LocalJobQueue::new(registry_with_noop("test.noop"));

        queue.submit(&mut project, "track_generated").unwrap();
        let error = queue.submit(&mut project, "track_generated").unwrap_err();

        assert!(error.to_string().contains("already pending or running"));
    }

    #[test]
    fn jobs_cancel_pending_marks_track_and_run_cancelled_without_markers() {
        let mut project = project_with_generated_track("test.cancel");
        let mut queue =
            LocalJobQueue::with_clock(registry_with_noop("test.cancel"), deterministic_clock());

        let job_id = queue.submit(&mut project, "track_generated").unwrap();
        queue.cancel(&job_id).unwrap();
        queue.run_next(&mut project).unwrap();

        assert_eq!(
            track_state(&project, "track_generated"),
            ResultState::Cancelled
        );
        assert_eq!(job_state(&project, &job_id), ResultState::Cancelled);
        assert!(project.markers.is_empty());
    }

    #[test]
    fn jobs_failed_transform_records_error_without_partial_markers() {
        let mut registry = JobRegistry::default();
        registry
            .register(test_spec("test.fail"), |_context, _params| {
                Err(TransformRunError::Failed("old job failed".to_string()))
            })
            .unwrap();
        let mut project = project_with_generated_track("test.fail");
        project
            .markers
            .push(marker("old_marker", "track_generated", 9.0));
        let mut queue = LocalJobQueue::with_clock(registry, deterministic_clock());

        let job_id = queue.submit(&mut project, "track_generated").unwrap();
        queue.run_next(&mut project).unwrap();

        assert_eq!(
            track_state(&project, "track_generated"),
            ResultState::Failed
        );
        assert_eq!(job_state(&project, &job_id), ResultState::Failed);
        assert!(track_by_id(&project, "track_generated")
            .error
            .contains("old job failed"));
        assert_eq!(project.markers[0].id, "old_marker");
    }

    #[test]
    fn jobs_failed_rerun_marks_generated_and_editable_dependents_stale() {
        let mut registry = JobRegistry::default();
        registry
            .register(test_spec("test.fail"), |_context, _params| {
                Err(TransformRunError::Failed("old job failed".to_string()))
            })
            .unwrap();
        let mut project = project_with_generated_track("test.fail");
        project.tracks.extend([
            generated_child("track_downstream", "track_generated"),
            editable_child("track_edit", "track_generated"),
        ]);
        let mut queue = LocalJobQueue::with_clock(registry, deterministic_clock());

        let job_id = queue.submit(&mut project, "track_generated").unwrap();
        queue.run_next(&mut project).unwrap();

        assert_eq!(job_state(&project, &job_id), ResultState::Failed);
        assert_eq!(
            track_state(&project, "track_generated"),
            ResultState::Failed
        );
        assert_eq!(
            track_state(&project, "track_downstream"),
            ResultState::Stale
        );
        assert_eq!(track_state(&project, "track_edit"), ResultState::Stale);
    }

    #[test]
    fn jobs_cancelled_rerun_marks_generated_and_editable_dependents_stale() {
        let mut project = project_with_generated_track("test.noop");
        project.tracks.extend([
            generated_child("track_downstream", "track_generated"),
            editable_child("track_edit", "track_generated"),
        ]);
        let mut queue = LocalJobQueue::new(registry_with_noop("test.noop"));

        let job_id = queue.submit(&mut project, "track_generated").unwrap();
        queue.cancel(&job_id).unwrap();
        queue.run_next(&mut project).unwrap();

        assert_eq!(job_state(&project, &job_id), ResultState::Cancelled);
        assert_eq!(
            track_state(&project, "track_generated"),
            ResultState::Cancelled
        );
        assert_eq!(
            track_state(&project, "track_downstream"),
            ResultState::Stale
        );
        assert_eq!(track_state(&project, "track_edit"), ResultState::Stale);
    }

    #[test]
    fn jobs_malformed_marker_output_leaves_no_partial_markers() {
        let mut registry = JobRegistry::default();
        registry
            .register(test_spec("test.bad_marker"), |_context, _params| {
                Ok(TransformResult::markers(vec![
                    ProducedMarker::new(0.0, "valid"),
                    ProducedMarker::new(f64::NAN, "invalid"),
                ]))
            })
            .unwrap();
        let mut project = project_with_generated_track("test.bad_marker");
        let mut queue = LocalJobQueue::with_clock(registry, deterministic_clock());

        let job_id = queue.submit(&mut project, "track_generated").unwrap();
        let error = queue.run_next(&mut project).unwrap_err();

        assert!(error.to_string().contains("finite"));
        assert_eq!(job_state(&project, &job_id), ResultState::Failed);
        assert_eq!(
            track_state(&project, "track_generated"),
            ResultState::Failed
        );
        assert!(track_by_id(&project, "track_generated")
            .error
            .contains("finite"));
        assert!(project.markers.is_empty());
        assert!(queue.submit(&mut project, "track_generated").is_ok());
    }

    #[test]
    fn jobs_negative_marker_timestamp_leaves_no_partial_markers() {
        let mut registry = JobRegistry::default();
        registry
            .register(test_spec("test.negative_marker"), |_context, _params| {
                Ok(TransformResult::markers(vec![
                    ProducedMarker::new(0.0, "valid"),
                    ProducedMarker::new(-0.1, "invalid"),
                ]))
            })
            .unwrap();
        let mut project = project_with_generated_track("test.negative_marker");
        let mut queue = LocalJobQueue::with_clock(registry, deterministic_clock());

        let job_id = queue.submit(&mut project, "track_generated").unwrap();
        let error = queue.run_next(&mut project).unwrap_err();

        assert!(error.to_string().contains("non-negative"));
        assert_eq!(job_state(&project, &job_id), ResultState::Failed);
        assert_eq!(
            track_state(&project, "track_generated"),
            ResultState::Failed
        );
        assert!(track_by_id(&project, "track_generated")
            .error
            .contains("non-negative"));
        assert!(project.markers.is_empty());
        assert!(queue.submit(&mut project, "track_generated").is_ok());
    }

    #[test]
    fn jobs_invalid_artifact_failure_does_not_leave_track_running() {
        let mut registry = JobRegistry::default();
        registry
            .register(test_spec("test.bad_artifact"), |_context, _params| {
                Ok(TransformResult::artifact("../bad", b"bad artifact"))
            })
            .unwrap();
        let mut project = project_with_generated_track("test.bad_artifact");
        let mut queue = LocalJobQueue::with_clock(registry, deterministic_clock());

        let job_id = queue.submit(&mut project, "track_generated").unwrap();
        let error = queue.run_next(&mut project).unwrap_err();

        assert!(error.to_string().contains("invalid artifact kind"));
        assert_eq!(job_state(&project, &job_id), ResultState::Failed);
        assert_eq!(
            track_state(&project, "track_generated"),
            ResultState::Failed
        );
        assert!(project.cache_entries.is_empty());
        assert!(queue.submit(&mut project, "track_generated").is_ok());
    }

    #[test]
    fn jobs_recomputes_dependency_hash_from_parent_cache_refs() {
        let mut project = project_with_generated_track("test.noop");
        project.tracks[0].cache_refs = vec!["cache_new".to_string()];
        let mut queue = LocalJobQueue::new(registry_with_noop("test.noop"));

        queue.submit(&mut project, "track_generated").unwrap();

        let expected = track_dependency_hash(
            &["cache_new".to_string()],
            "test.noop",
            "1",
            &JsonObject::new(),
        )
        .unwrap();
        assert_eq!(
            track_by_id(&project, "track_generated").dependency_hash,
            expected
        );
    }

    #[test]
    fn jobs_success_marks_generated_and_editable_dependents_stale() {
        let mut project = project_with_generated_track("test.noop");
        project.tracks.extend([
            generated_child("track_downstream", "track_generated"),
            editable_child("track_edit", "track_generated"),
        ]);
        let mut queue = LocalJobQueue::new(registry_with_noop("test.noop"));

        let job_id = queue.submit(&mut project, "track_generated").unwrap();
        queue.run_next(&mut project).unwrap();

        assert_eq!(job_state(&project, &job_id), ResultState::Complete);
        assert_eq!(
            track_state(&project, "track_downstream"),
            ResultState::Stale
        );
        assert_eq!(track_state(&project, "track_edit"), ResultState::Stale);
    }

    #[test]
    fn jobs_refresh_cache_validity_marks_invalid_refs_and_dependents_stale() {
        let mut project = project_with_generated_track("test.noop");
        track_by_id_mut(&mut project, "track_generated").cache_refs =
            vec!["cache_missing".to_string()];
        project
            .cache_entries
            .push(cache_entry("cache_missing", "stem"));
        project
            .tracks
            .push(generated_child("track_downstream", "track_generated"));
        let mut queue = LocalJobQueue::new(registry_with_noop("test.noop"));

        let invalid = queue.refresh_cache_validity(&mut project, |_| false);

        assert_eq!(invalid, ["cache_missing"]);
        assert_eq!(project.cache_entries[0].validation_status, "invalid");
        assert_eq!(track_state(&project, "track_generated"), ResultState::Stale);
        assert_eq!(
            track_state(&project, "track_downstream"),
            ResultState::Stale
        );
    }

    #[test]
    fn jobs_runtime_params_cannot_change_cached_identity() {
        let mut project = project_with_generated_track("test.noop");
        track_by_id_mut(&mut project, "track_generated")
            .transform_params
            .insert("timestamp".to_string(), json!(1.0));
        let mut queue = LocalJobQueue::new(registry_with_noop("test.noop"));

        let error = queue
            .submit_with_runtime_params(
                &mut project,
                "track_generated",
                object(json!({"timestamp": 2.0})),
            )
            .unwrap_err();

        assert!(error.to_string().contains("runtime transform params"));
        assert!(project.job_runs.is_empty());
    }

    #[test]
    fn jobs_runtime_audio_path_does_not_change_cached_identity() {
        let mut project = project_with_generated_track("test.noop");
        track_by_id_mut(&mut project, "track_generated")
            .transform_params
            .insert("audio_path".to_string(), json!("/old.wav"));
        let mut queue = LocalJobQueue::new(registry_with_noop("test.noop"));

        let job_id = queue
            .submit_with_runtime_params(
                &mut project,
                "track_generated",
                object(json!({"audio_path": "/new.wav"})),
            )
            .unwrap();

        assert_eq!(job_state(&project, &job_id), ResultState::Pending);
        assert!(!track_by_id(&project, "track_generated")
            .dependency_hash
            .is_empty());
    }

    #[test]
    fn jobs_seed_ids_from_persisted_job_runs() {
        let mut project = project_with_generated_track("test.noop");
        let mut first_queue =
            LocalJobQueue::with_clock(registry_with_noop("test.noop"), deterministic_clock());

        let first_job_id = first_queue.submit(&mut project, "track_generated").unwrap();
        first_queue.run_next(&mut project).unwrap();
        let mut fresh_queue =
            LocalJobQueue::with_clock(registry_with_noop("test.noop"), deterministic_clock());
        let second_job_id = fresh_queue.submit(&mut project, "track_generated").unwrap();

        assert_eq!(first_job_id, "job_0001");
        assert_eq!(second_job_id, "job_0002");
        let unique_ids = project
            .job_runs
            .iter()
            .map(|run| run.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique_ids.len(), project.job_runs.len());
    }

    #[test]
    fn jobs_runtime_only_params_reach_runner_without_changing_saved_params() {
        let mut registry = JobRegistry::default();
        registry
            .register(test_spec("test.runtime_param"), |_context, params| {
                let label = params
                    .get("audio_path")
                    .and_then(Value::as_str)
                    .unwrap_or("missing")
                    .to_string();
                Ok(TransformResult::markers(vec![ProducedMarker::new(
                    0.0, label,
                )]))
            })
            .unwrap();
        let mut project = project_with_generated_track("test.runtime_param");
        track_by_id_mut(&mut project, "track_generated")
            .transform_params
            .insert("audio_path".to_string(), json!("/old.wav"));
        let mut queue = LocalJobQueue::new(registry);

        let job_id = queue
            .submit_with_runtime_params(
                &mut project,
                "track_generated",
                object(json!({"audio_path": "/new.wav"})),
            )
            .unwrap();
        queue.run_next(&mut project).unwrap();

        assert_eq!(project.markers[0].label, "/new.wav");
        assert_eq!(
            track_by_id(&project, "track_generated").transform_params["audio_path"],
            json!("/old.wav")
        );
        assert_eq!(
            run_by_id(&project, &job_id).parameters["audio_path"],
            "/new.wav"
        );
    }

    #[test]
    fn jobs_reject_rerun_when_input_track_is_stale() {
        let mut project = project_with_generated_track("test.noop");
        track_by_id_mut(&mut project, "track_source").result_state = ResultState::Stale;
        track_by_id_mut(&mut project, "track_source").error =
            "input audio asset offline: source.wav".to_string();
        let mut queue = LocalJobQueue::new(registry_with_noop("test.noop"));

        let error = queue.submit(&mut project, "track_generated").unwrap_err();

        assert!(error.to_string().contains("input track is not complete"));
        assert!(project.job_runs.is_empty());
        assert_eq!(
            track_state(&project, "track_generated"),
            ResultState::Complete
        );
    }

    fn registry_with_noop(transform_id: &str) -> JobRegistry {
        let mut registry = JobRegistry::default();
        registry
            .register(test_spec(transform_id), |_context, _params| {
                Ok(TransformResult::default())
            })
            .unwrap();
        registry
    }

    fn test_spec(transform_id: &str) -> TransformSpec {
        TransformSpec::new(
            transform_id,
            "1",
            "Test Transform",
            "audio-or-markers.v1",
            "markers.v1",
            "light",
        )
    }

    fn project_with_generated_track(transform_id: &str) -> ProjectDocument {
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
        project.tracks.extend([
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
                provenance: object(json!({"asset_id": "asset_source"})),
                error: String::new(),
            },
            Track {
                id: "track_generated".to_string(),
                track_type: TrackType::Generated,
                name: "Generated".to_string(),
                input_track_ids: vec!["track_source".to_string()],
                transform_id: transform_id.to_string(),
                transform_params: JsonObject::new(),
                transform_version: "1".to_string(),
                output_schema: "markers.v1".to_string(),
                dependency_hash: "old_dep".to_string(),
                result_state: ResultState::Complete,
                cache_refs: Vec::new(),
                provenance: JsonObject::new(),
                error: String::new(),
            },
        ]);
        project
    }

    fn generated_child(id: &str, parent_id: &str) -> Track {
        Track {
            id: id.to_string(),
            track_type: TrackType::Generated,
            name: id.to_string(),
            input_track_ids: vec![parent_id.to_string()],
            transform_id: "test.child".to_string(),
            transform_params: JsonObject::new(),
            transform_version: "1".to_string(),
            output_schema: "markers.v1".to_string(),
            dependency_hash: "child_dep".to_string(),
            result_state: ResultState::Complete,
            cache_refs: Vec::new(),
            provenance: JsonObject::new(),
            error: String::new(),
        }
    }

    fn editable_child(id: &str, parent_id: &str) -> Track {
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
            provenance: JsonObject::new(),
            error: String::new(),
        }
    }

    fn marker(id: &str, track_id: &str, timestamp: f64) -> Marker {
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

    fn cache_entry(id: &str, artifact_kind: &str) -> CacheEntry {
        CacheEntry {
            id: id.to_string(),
            dependency_hash: "dep".to_string(),
            artifact_kind: artifact_kind.to_string(),
            path: format!("{artifact_kind}/{id}.bin"),
            created_at: String::new(),
            transform_version: "1".to_string(),
            size_bytes: 0,
            payload_digest: String::new(),
            validation_status: "valid".to_string(),
        }
    }

    fn deterministic_clock() -> impl FnMut() -> String {
        let mut tick = 0;
        move || {
            tick += 1;
            format!("tick-{tick}")
        }
    }

    fn track_by_id<'a>(project: &'a ProjectDocument, track_id: &str) -> &'a Track {
        project
            .tracks
            .iter()
            .find(|track| track.id == track_id)
            .unwrap()
    }

    fn track_by_id_mut<'a>(project: &'a mut ProjectDocument, track_id: &str) -> &'a mut Track {
        project
            .tracks
            .iter_mut()
            .find(|track| track.id == track_id)
            .unwrap()
    }

    fn run_by_id<'a>(
        project: &'a ProjectDocument,
        job_id: &str,
    ) -> &'a autolight_core::project::JobRun {
        project
            .job_runs
            .iter()
            .find(|run| run.id == job_id)
            .unwrap()
    }

    fn track_state(project: &ProjectDocument, track_id: &str) -> ResultState {
        track_by_id(project, track_id).result_state
    }

    fn job_state(project: &ProjectDocument, job_id: &str) -> ResultState {
        run_by_id(project, job_id).state
    }

    fn object(value: Value) -> JsonObject {
        value.as_object().cloned().unwrap()
    }

    fn test_dir(name: &str) -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "autolight-jobs-{name}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }
}
