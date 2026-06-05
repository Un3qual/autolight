use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use autolight_core::project::{CacheEntry, JobRun, Marker, ProjectDocument, Track};
use autolight_jobs::queue::{LocalJobQueue, ProgressReporter, TransformCancellationToken};

use super::jobs::job_registry;

pub(super) type SharedJobProgress = Arc<Mutex<BTreeMap<String, f64>>>;

pub(super) struct JobWorker {
    job_id: String,
    handle: JobWorkerHandle,
}

enum JobWorkerHandle {
    Thread(JoinHandle<JobWorkerResult>),
    #[cfg(test)]
    Ready(Box<JobWorkerResult>),
}

impl JobWorker {
    pub(super) fn job_id(&self) -> &str {
        &self.job_id
    }

    pub(super) fn is_finished(&self) -> bool {
        match &self.handle {
            JobWorkerHandle::Thread(handle) => handle.is_finished(),
            #[cfg(test)]
            JobWorkerHandle::Ready(_) => true,
        }
    }

    pub(super) fn join(self) -> Result<JobWorkerResult, String> {
        match self.handle {
            JobWorkerHandle::Thread(handle) => handle
                .join()
                .map_err(|_| format!("job worker panicked: {}", self.job_id)),
            #[cfg(test)]
            JobWorkerHandle::Ready(result) => Ok(*result),
        }
    }
}

pub(super) struct JobWorkerResult {
    pub(super) job_id: String,
    pub(super) track_id: String,
    pub(super) track: Track,
    pub(super) job_run: JobRun,
    pub(super) markers: Vec<Marker>,
    pub(super) cache_entries: Vec<CacheEntry>,
    pub(super) artifact_dir: Option<PathBuf>,
    pub(super) error: Option<String>,
}

pub(super) fn new_shared_job_progress() -> SharedJobProgress {
    Arc::new(Mutex::new(BTreeMap::new()))
}

pub(super) fn spawn_job_worker(
    project: ProjectDocument,
    job_id: String,
    artifact_dir: Option<PathBuf>,
    token: TransformCancellationToken,
    progress: SharedJobProgress,
) -> JobWorker {
    let worker_job_id = job_id.clone();
    let handle =
        thread::spawn(move || run_job_worker(project, job_id, artifact_dir, token, progress));
    JobWorker {
        job_id: worker_job_id,
        handle: JobWorkerHandle::Thread(handle),
    }
}

#[cfg(test)]
pub(super) fn ready_job_worker_for_run(
    project: ProjectDocument,
    job_id: String,
    artifact_dir: Option<PathBuf>,
    token: TransformCancellationToken,
    progress: SharedJobProgress,
) -> JobWorker {
    let worker_job_id = job_id.clone();
    let result = run_job_worker(project, job_id, artifact_dir, token, progress);
    JobWorker {
        job_id: worker_job_id,
        handle: JobWorkerHandle::Ready(Box::new(result)),
    }
}

#[cfg(test)]
pub(super) fn ready_failed_job_worker(
    job_id: String,
    artifact_dir: Option<PathBuf>,
    error: impl Into<String>,
) -> JobWorker {
    let worker_job_id = job_id.clone();
    JobWorker {
        job_id: worker_job_id,
        handle: JobWorkerHandle::Ready(Box::new(fallback_failed_worker_result(
            job_id,
            artifact_dir,
            error,
        ))),
    }
}

fn run_job_worker(
    mut project: ProjectDocument,
    job_id: String,
    artifact_dir: Option<PathBuf>,
    token: TransformCancellationToken,
    progress: SharedJobProgress,
) -> JobWorkerResult {
    let progress_reporter = progress_reporter(Arc::clone(&progress));
    let mut queue = LocalJobQueue::new(job_registry());
    let run_result = queue.run_detached_job_with_artifact_dir(
        &mut project,
        &job_id,
        artifact_dir.as_deref(),
        token,
        Some(progress_reporter),
    );
    let error = run_result.err().map(|error| error.to_string());
    match result_from_project(&project, &job_id, artifact_dir.clone(), error.clone()) {
        Some(result) => result,
        None => fallback_failed_worker_result(
            job_id,
            artifact_dir,
            error.unwrap_or_else(|| "job disappeared while running".to_string()),
        ),
    }
}

pub(super) fn fallback_failed_worker_result(
    job_id: String,
    artifact_dir: Option<PathBuf>,
    error: impl Into<String>,
) -> JobWorkerResult {
    let error = error.into();
    JobWorkerResult {
        job_id: job_id.clone(),
        track_id: String::default(),
        track: Track {
            id: String::default(),
            track_type: autolight_core::project::TrackType::Generated,
            name: String::default(),
            input_track_ids: Vec::default(),
            transform_id: String::default(),
            transform_params: autolight_core::project::JsonObject::default(),
            transform_version: String::default(),
            output_schema: String::default(),
            dependency_hash: String::default(),
            result_state: autolight_core::project::ResultState::Failed,
            cache_refs: Vec::default(),
            provenance: autolight_core::project::JsonObject::default(),
            error: error.clone(),
        },
        job_run: JobRun {
            id: job_id,
            track_id: String::default(),
            transform_id: String::default(),
            transform_version: String::default(),
            parameters_hash: String::default(),
            parameters: autolight_core::project::JsonObject::default(),
            state: autolight_core::project::ResultState::Failed,
            progress: 1.0,
            started_at: String::default(),
            completed_at: String::default(),
            error: error.clone(),
            produced_cache_refs: Vec::default(),
        },
        markers: Vec::default(),
        cache_entries: Vec::default(),
        artifact_dir,
        error: Some(error),
    }
}

fn progress_reporter(progress: SharedJobProgress) -> ProgressReporter {
    Arc::new(move |job_id, _track_id, value| {
        if let Ok(mut progress) = progress.lock() {
            progress.insert(job_id.to_string(), value);
        }
    })
}

fn result_from_project(
    project: &ProjectDocument,
    job_id: &str,
    artifact_dir: Option<PathBuf>,
    error: Option<String>,
) -> Option<JobWorkerResult> {
    let job_run = project
        .job_runs
        .iter()
        .find(|run| run.id == job_id)?
        .clone();
    let track = project
        .tracks
        .iter()
        .find(|track| track.id == job_run.track_id)?
        .clone();
    let markers = project
        .markers
        .iter()
        .filter(|marker| marker.track_id == job_run.track_id)
        .cloned()
        .collect();
    let cache_entries = project
        .cache_entries
        .iter()
        .filter(|entry| job_run.produced_cache_refs.contains(&entry.id))
        .cloned()
        .collect();
    Some(JobWorkerResult {
        job_id: job_id.to_string(),
        track_id: job_run.track_id.clone(),
        track,
        job_run,
        markers,
        cache_entries,
        artifact_dir,
        error,
    })
}
