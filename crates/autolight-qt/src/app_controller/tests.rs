use serde_json::Value;

use super::job_worker::JobWorkerResult;
use super::project_io::cache_entry_path_is_safe;
use super::{path_from_qml, AppControllerState, SMOKE_PROJECT_NAME, WAVE_SUBFORMAT_PCM};
use crate::timeline_renderer::cache::AnalysisRenderRequest;
use crate::timeline_renderer::waveform::WaveformRenderRequest;
use autolight_core::cache::cache_entry_for_bytes;
use autolight_core::project::{
    CacheValidationStatus, ImportStatus, JobRun, ProjectDocument, ResultState, Track, TrackType,
};
use autolight_core::transforms::TransformSpec;
use autolight_jobs::queue::{JobRegistry, LocalJobQueue, ProducedMarker, TransformResult};

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
    assert!(state.selected_track_can_play);
    assert!(state.project.job_runs.is_empty());
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
fn controller_demo_energy_track_does_not_expose_orphan_cancel() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();

    state.select_track_state("track_drum_energy");
    state.cancel_selected_job_state();

    assert!(!state.selected_track_has_running_job);
    assert!(!state.last_error.to_string().contains("job not found"));
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
fn controller_submit_track_returns_before_worker_poll_commits_job() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    let track_id = state.add_transform_track_state(
        "track_source",
        "markers.fixed_interval",
        "1",
        r#"{"duration": 1.0, "interval": 0.5}"#,
    );

    let job_id = state.submit_track_state(&track_id);

    assert!(!job_id.is_empty());
    assert!(state
        .job_workers
        .iter()
        .any(|worker| worker.job_id() == job_id));
    assert!(state
        .project
        .job_runs
        .iter()
        .any(|run| run.id == job_id && run.state == ResultState::Pending));

    for _ in 0..100 {
        if state.poll_job_workers_state() > 0
            && state
                .project
                .job_runs
                .iter()
                .any(|run| run.id == job_id && run.state == ResultState::Complete)
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let track = state
        .project
        .tracks
        .iter()
        .find(|track| track.id == track_id)
        .unwrap();
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
    assert!(state.job_workers.is_empty());
}

#[test]
fn controller_submit_artifact_track_requires_cache_directory_before_creating_job() {
    let root = test_dir("unsaved-artifact-submit");
    let audio_path = root.join("song.wav");
    write_test_wav(&audio_path, 8_000, 1, 16_000);
    let mut state = AppControllerState::default();
    let source_id = state.import_audio_state(audio_path.to_str().unwrap());
    let track_id = state.add_transform_track_state(&source_id, "waveform.summary", "1", "{}");

    let job_id = state.submit_track_state(&track_id);

    assert!(job_id.is_empty());
    assert!(state.project.job_runs.is_empty());
    assert!(state.job_workers.is_empty());
    assert!(state
        .last_error
        .to_string()
        .contains("save the project before running artifact-producing transforms"));
}

#[test]
fn controller_worker_progress_preserves_manual_edit_history() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    state.select_track_state("track_edit");
    let marker_id =
        state.add_marker_to_selected_track_with_duration_state(1.25, 0.5, "Cue", "cue", "cyan");
    state.project.job_runs.push(JobRun {
        id: "job_progress".to_string(),
        track_id: "track_beats".to_string(),
        transform_id: "markers.fixed_interval".to_string(),
        transform_version: "1".to_string(),
        parameters_hash: "dep_beats".to_string(),
        parameters: serde_json::Map::default(),
        state: ResultState::Running,
        progress: 0.1,
        started_at: String::default(),
        completed_at: String::default(),
        error: String::default(),
        produced_cache_refs: Vec::default(),
    });
    state
        .job_progress
        .lock()
        .unwrap()
        .insert("job_progress".to_string(), 0.5);

    assert_eq!(state.poll_job_workers_state(), 1);

    assert!(state.can_undo);
    assert!(state.undo_state());
    assert!(!state
        .project
        .markers
        .iter()
        .any(|marker| marker.id == marker_id));
}

#[test]
fn controller_async_worker_merge_preserves_current_stale_track_with_same_dependency_hash() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    let track_index = state
        .project
        .tracks
        .iter()
        .position(|track| track.id == "track_beats")
        .unwrap();
    state.project.tracks[track_index].dependency_hash = "same-hash".to_string();
    state.project.tracks[track_index].result_state = ResultState::Stale;
    state.project.tracks[track_index].error = "input changed".to_string();
    state.project.job_runs.push(JobRun {
        id: "job_async".to_string(),
        track_id: "track_beats".to_string(),
        transform_id: "markers.fixed_interval".to_string(),
        transform_version: "1".to_string(),
        parameters_hash: "same-hash".to_string(),
        parameters: serde_json::Map::default(),
        state: ResultState::Pending,
        progress: 0.0,
        started_at: String::default(),
        completed_at: String::default(),
        error: String::default(),
        produced_cache_refs: Vec::default(),
    });
    let mut completed_track = state.project.tracks[track_index].clone();
    completed_track.result_state = ResultState::Complete;
    completed_track.error.clear();
    let original_marker_ids = state
        .project
        .markers
        .iter()
        .filter(|marker| marker.track_id == "track_beats")
        .map(|marker| marker.id.clone())
        .collect::<Vec<_>>();

    state.merge_job_worker_result(JobWorkerResult {
        job_id: "job_async".to_string(),
        track_id: "track_beats".to_string(),
        track: completed_track,
        job_run: JobRun {
            id: "job_async".to_string(),
            track_id: "track_beats".to_string(),
            transform_id: "markers.fixed_interval".to_string(),
            transform_version: "1".to_string(),
            parameters_hash: "same-hash".to_string(),
            parameters: serde_json::Map::default(),
            state: ResultState::Complete,
            progress: 1.0,
            started_at: String::default(),
            completed_at: String::default(),
            error: String::default(),
            produced_cache_refs: Vec::default(),
        },
        markers: Vec::default(),
        cache_entries: Vec::default(),
        artifact_dir: None,
        error: None,
    });

    let track = state
        .project
        .tracks
        .iter()
        .find(|track| track.id == "track_beats")
        .unwrap();
    assert_eq!(track.result_state, ResultState::Stale);
    assert_eq!(track.error, "input changed");
    assert_eq!(
        state
            .project
            .job_runs
            .iter()
            .find(|run| run.id == "job_async")
            .unwrap()
            .state,
        ResultState::Stale
    );
    assert_eq!(
        state
            .project
            .markers
            .iter()
            .filter(|marker| marker.track_id == "track_beats")
            .map(|marker| marker.id.clone())
            .collect::<Vec<_>>(),
        original_marker_ids
    );
}

#[test]
fn controller_async_worker_join_error_finalizes_active_job() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    state.project.job_runs.push(JobRun {
        id: "job_panicked".to_string(),
        track_id: "track_beats".to_string(),
        transform_id: "markers.fixed_interval".to_string(),
        transform_version: "1".to_string(),
        parameters_hash: "dep_beats".to_string(),
        parameters: serde_json::Map::default(),
        state: ResultState::Running,
        progress: 0.5,
        started_at: String::default(),
        completed_at: String::default(),
        error: String::default(),
        produced_cache_refs: Vec::default(),
    });
    state
        .project
        .tracks
        .iter_mut()
        .find(|track| track.id == "track_beats")
        .unwrap()
        .result_state = ResultState::Running;

    state.finalize_worker_join_error("job_panicked", "worker thread panicked");

    let run = state
        .project
        .job_runs
        .iter()
        .find(|run| run.id == "job_panicked")
        .unwrap();
    assert_eq!(run.state, ResultState::Failed);
    assert_eq!(run.error, "worker thread panicked");
    let track = state
        .project
        .tracks
        .iter()
        .find(|track| track.id == "track_beats")
        .unwrap();
    assert_eq!(track.result_state, ResultState::Failed);
    assert_eq!(track.error, "worker thread panicked");
}

#[test]
fn controller_reset_cancels_and_drains_running_workers() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    let track_id = state.add_transform_track_state(
        "track_source",
        "markers.fixed_interval",
        "1",
        r#"{"duration": 99.0, "interval": 0.001}"#,
    );
    let job_id = state.submit_track_state(&track_id);

    state.reset_job_runtime_state();

    assert!(state.job_workers.is_empty());
    assert_eq!(
        state
            .project
            .job_runs
            .iter()
            .find(|run| run.id == job_id)
            .unwrap()
            .state,
        ResultState::Cancelled
    );
    assert_eq!(
        state
            .project
            .tracks
            .iter()
            .find(|track| track.id == track_id)
            .unwrap()
            .result_state,
        ResultState::Cancelled
    );
}

#[test]
fn controller_run_waveform_summary_completes_with_visible_waveform() {
    let root = test_dir("unsupported-transform");
    let project_path = root.join("show.autolight");
    let audio_path = root.join("song.wav");
    write_test_wav(&audio_path, 8_000, 1, 8_000);
    let mut state = AppControllerState {
        project_path: cxx_qt_lib::QString::from(project_path.to_string_lossy().to_string()),
        ..Default::default()
    };
    let source_track_id = state.import_audio_state(audio_path.to_str().unwrap());
    let track_id = state.add_transform_track_state(
        &source_track_id,
        "waveform.summary",
        "1",
        r#"{"buckets": 4}"#,
    );

    let job_id = state.run_track_state(&track_id);

    assert!(!job_id.is_empty());
    let track = state
        .project
        .tracks
        .iter()
        .find(|track| track.id == track_id)
        .unwrap();
    assert_eq!(track.result_state, ResultState::Complete);
    assert!(track.transform_params.get("audio_path").is_none());
    assert_eq!(track.cache_refs.len(), 1);
    assert!(track.provenance.get("waveform_payload").is_some());
    let visible_samples = track
        .provenance
        .get("visible_waveform")
        .and_then(|value| value.get("samples"))
        .and_then(Value::as_array)
        .unwrap();
    assert!(!visible_samples.is_empty());
    let entry = state
        .project
        .cache_entries
        .iter()
        .find(|entry| entry.id == track.cache_refs[0])
        .unwrap();
    assert_eq!(entry.artifact_kind, "waveform");
    assert!(root.join(&entry.path).is_file());
    let run = state
        .project
        .job_runs
        .iter()
        .find(|run| run.id == job_id)
        .unwrap();
    assert_eq!(
        run.parameters["audio_path"].as_str(),
        Some(audio_path.to_string_lossy().as_ref())
    );
    assert!(state
        .project
        .job_runs
        .iter()
        .any(|run| run.id == job_id && run.state == ResultState::Complete));
    let rows = json_array(&state.timeline_rows_json.to_string());
    let row = rows.iter().find(|row| row["trackId"] == track_id).unwrap();
    assert_eq!(row["waveformRef"]["trackId"], track_id);
    assert_eq!(row["waveformRef"]["artifactKind"], "waveform");
    assert!(row.get("waveformLevels").is_none());
    assert!(row.get("visibleWaveformSamples").is_none());
}

#[test]
fn controller_waveform_summary_uses_parent_audio_artifact_path_for_generated_parent() {
    let root = test_dir("waveform-parent-audio-artifact");
    let project_path = root.join("show.autolight");
    let source_path = root.join("source.wav");
    let stem_payload_path = root.join("stem-source.wav");
    write_test_wav(&source_path, 8_000, 1, 8_000);
    write_test_wav(&stem_payload_path, 8_000, 1, 4_000);
    let stem_payload = std::fs::read(&stem_payload_path).unwrap();
    let mut stem_entry =
        cache_entry_for_bytes("stem", "stem-hash", &stem_payload, "1", "test").unwrap();
    let stem_artifact_path = root.join(&stem_entry.path);
    std::fs::create_dir_all(stem_artifact_path.parent().unwrap()).unwrap();
    std::fs::write(&stem_artifact_path, &stem_payload).unwrap();
    let mut state = AppControllerState {
        project_path: cxx_qt_lib::QString::from(project_path.to_string_lossy().to_string()),
        ..Default::default()
    };
    let source_track_id = state.import_audio_state(source_path.to_str().unwrap());
    stem_entry.validation_status = CacheValidationStatus::Valid;
    let stem_cache_ref = stem_entry.id.clone();
    state.project.cache_entries.push(stem_entry);
    state.project.tracks.push(Track {
        id: "track_stem".to_string(),
        track_type: TrackType::Generated,
        name: "Stem".to_string(),
        input_track_ids: vec![source_track_id],
        transform_id: "test.stem".to_string(),
        transform_params: serde_json::Map::default(),
        transform_version: "1".to_string(),
        output_schema: "artifact.stem.v1".to_string(),
        dependency_hash: "stem-hash".to_string(),
        result_state: ResultState::Complete,
        cache_refs: vec![stem_cache_ref],
        provenance: serde_json::Map::default(),
        error: String::default(),
    });
    let waveform_track_id =
        state.add_transform_track_state("track_stem", "waveform.summary", "1", r#"{"buckets": 4}"#);

    let job_id = state.run_track_state(&waveform_track_id);

    assert!(!job_id.is_empty());
    let run = state
        .project
        .job_runs
        .iter()
        .find(|run| run.id == job_id)
        .unwrap();
    assert_eq!(
        run.parameters["audio_path"].as_str(),
        Some(stem_artifact_path.to_string_lossy().as_ref())
    );
}

#[test]
fn controller_rejects_unrunnable_builtin_transform_before_track_creation() {
    let root = test_dir("unsupported-transform");
    let project_path = root.join("show.autolight");
    let audio_path = root.join("song.wav");
    write_test_wav(&audio_path, 8_000, 1, 8_000);
    let mut state = AppControllerState {
        project_path: cxx_qt_lib::QString::from(project_path.to_string_lossy().to_string()),
        ..Default::default()
    };
    let source_track_id = state.import_audio_state(audio_path.to_str().unwrap());
    let track_id = state.add_transform_track_state(&source_track_id, "timing.beats", "1", "{}");

    assert!(track_id.is_empty());
    assert!(state.last_error.to_string().contains("not available"));
    assert!(state
        .project
        .tracks
        .iter()
        .all(|track| track.transform_id != "timing.beats"));
}

#[test]
fn controller_run_completion_error_refreshes_failed_state() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    let spec = TransformSpec::new(
        "test.bad_marker",
        "1",
        "Bad Marker",
        "audio-or-markers.v1",
        "markers.v1",
        "light",
    );
    state.transform_registry.register(spec.clone()).unwrap();
    let mut registry = JobRegistry::default();
    registry
        .register(spec, |_context, _params| {
            Ok(TransformResult::markers(vec![
                ProducedMarker::new(0.0, "valid"),
                ProducedMarker::new(-0.1, "invalid"),
            ]))
        })
        .unwrap();
    state.job_queue = LocalJobQueue::new(registry);
    let track_id = state.add_transform_track_state("track_source", "test.bad_marker", "1", "{}");
    state.mark_clean();

    let job_id = state.run_track_state(&track_id);

    assert!(job_id.is_empty());
    assert!(state.is_dirty);
    let track = state
        .project
        .tracks
        .iter()
        .find(|track| track.id == track_id)
        .unwrap();
    assert_eq!(track.result_state, ResultState::Failed);
    assert!(track.error.contains("non-negative"));
    assert!(state
        .timeline_rows_json
        .to_string()
        .contains("\"resultState\":\"failed\""));
    assert!(state
        .project
        .job_runs
        .iter()
        .any(|run| run.track_id == track_id && run.state == ResultState::Failed));
}

#[test]
fn controller_run_track_persists_artifact_payloads_in_project_directory() {
    let root = test_dir("controller-artifact-cache");
    let project_path = root.join("show.autolight");
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    state.project_path = cxx_qt_lib::QString::from(project_path.to_string_lossy().to_string());
    let spec = TransformSpec::new(
        "test.artifact",
        "1",
        "Artifact",
        "audio-or-markers.v1",
        "artifact.stem.v1",
        "light",
    );
    state.transform_registry.register(spec.clone()).unwrap();
    let mut registry = JobRegistry::default();
    registry
        .register(spec, |_context, _params| {
            Ok(TransformResult::artifact("stem", b"cached stem"))
        })
        .unwrap();
    state.job_queue = LocalJobQueue::new(registry);
    let track_id = state.add_transform_track_state("track_source", "test.artifact", "1", "{}");

    let job_id = state.run_track_state(&track_id);

    assert!(!job_id.is_empty());
    let track = state
        .project
        .tracks
        .iter()
        .find(|track| track.id == track_id)
        .unwrap();
    assert_eq!(track.result_state, ResultState::Complete);
    let entry = state
        .project
        .cache_entries
        .iter()
        .find(|entry| entry.id == track.cache_refs[0])
        .unwrap();
    assert_eq!(
        std::fs::read(root.join(&entry.path)).unwrap(),
        b"cached stem"
    );
}

#[test]
fn controller_save_as_copies_cache_artifacts_to_new_project_directory() {
    let root = test_dir("controller-save-as-cache-copy");
    let source_path = root.join("source").join("show.autolight");
    let saved_path = root.join("copy").join("show-copy.autolight");
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    state.project_path = cxx_qt_lib::QString::from(source_path.to_string_lossy().to_string());
    let spec = TransformSpec::new(
        "test.artifact",
        "1",
        "Artifact",
        "audio-or-markers.v1",
        "artifact.stem.v1",
        "light",
    );
    state.transform_registry.register(spec.clone()).unwrap();
    let mut registry = JobRegistry::default();
    registry
        .register(spec, |_context, _params| {
            Ok(TransformResult::artifact("stem", b"cached stem"))
        })
        .unwrap();
    state.job_queue = LocalJobQueue::new(registry);
    let track_id = state.add_transform_track_state("track_source", "test.artifact", "1", "{}");
    assert!(!state.run_track_state(&track_id).is_empty());
    let entry_path = state
        .project
        .cache_entries
        .iter()
        .find(|entry| {
            state
                .project
                .tracks
                .iter()
                .find(|track| track.id == track_id)
                .unwrap()
                .cache_refs
                .contains(&entry.id)
        })
        .unwrap()
        .path
        .clone();

    assert!(state.save_project_state(saved_path.to_str().unwrap()));

    let copied_artifact = saved_path.parent().unwrap().join(&entry_path);
    assert_eq!(std::fs::read(&copied_artifact).unwrap(), b"cached stem");

    let mut reopened = AppControllerState::default();
    assert!(reopened.open_project_state(saved_path.to_str().unwrap()));
    assert_eq!(
        reopened
            .project
            .tracks
            .iter()
            .find(|track| track.id == track_id)
            .unwrap()
            .result_state,
        ResultState::Complete
    );
}

#[test]
fn controller_fixed_interval_rejects_unbounded_marker_generation() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    let track_id = state.add_transform_track_state(
        "track_source",
        "markers.fixed_interval",
        "1",
        r#"{"duration": 1000000.0, "interval": 0.001}"#,
    );

    let job_id = state.run_track_state(&track_id);

    assert!(!job_id.is_empty());
    let track = state
        .project
        .tracks
        .iter()
        .find(|track| track.id == track_id)
        .unwrap();
    assert_eq!(track.result_state, ResultState::Failed);
    assert!(track.error.contains("too many markers"));
    assert!(state
        .project
        .markers
        .iter()
        .all(|marker| marker.track_id != track_id));
}

#[test]
fn controller_fixed_interval_rejects_nonnumeric_params() {
    for (params, expected_error) in [
        (
            r#"{"duration": "8", "interval": 0.5}"#,
            "duration must be a number",
        ),
        (
            r#"{"duration": 1.0, "interval": "0.5"}"#,
            "interval must be a number",
        ),
    ] {
        let mut state = AppControllerState::default();
        state.load_demo_project_state();
        let track_id =
            state.add_transform_track_state("track_source", "markers.fixed_interval", "1", params);

        let job_id = state.run_track_state(&track_id);

        assert!(!job_id.is_empty());
        let track = state
            .project
            .tracks
            .iter()
            .find(|track| track.id == track_id)
            .unwrap();
        assert_eq!(track.result_state, ResultState::Failed);
        assert!(track.error.contains(expected_error));
        assert!(state
            .project
            .markers
            .iter()
            .all(|marker| marker.track_id != track_id));
    }
}

#[test]
fn controller_rerun_requires_complete_input_tracks() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    let track_id = state.add_transform_track_state(
        "track_source",
        "markers.fixed_interval",
        "1",
        r#"{"duration": 1.0, "interval": 0.5}"#,
    );
    state.run_track_state(&track_id);
    state.select_track_state(&track_id);
    assert!(state.selected_track_can_rerun);

    let source = state
        .project
        .tracks
        .iter_mut()
        .find(|track| track.id == "track_source")
        .unwrap();
    source.result_state = ResultState::Stale;
    source.error = "input audio asset offline: song.wav".to_string();
    state.refresh_view_state();
    assert!(!state.selected_track_can_rerun);

    let source = state
        .project
        .tracks
        .iter_mut()
        .find(|track| track.id == "track_source")
        .unwrap();
    source.result_state = ResultState::Complete;
    source.error.clear();
    state.refresh_view_state();
    assert!(state.selected_track_can_rerun);
}

#[test]
fn controller_demo_waveform_can_be_selected_and_rerun() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    state.select_track_state("track_waveform");

    assert_eq!(state.selected_track_id.to_string(), "track_waveform");
    assert!(state.selected_track_can_rerun);
    assert!(state.selected_track_can_play);

    let job_id = state.rerun_track_state("track_waveform");

    assert!(!job_id.is_empty());
    let waveform = state
        .project
        .tracks
        .iter()
        .find(|track| track.id == "track_waveform")
        .unwrap();
    assert_eq!(waveform.result_state, ResultState::Complete);
    assert!(waveform.provenance.get("visible_waveform").is_some());
    let rows = json_array(&state.timeline_rows_json.to_string());
    let row = rows
        .iter()
        .find(|row| row["trackId"] == "track_waveform")
        .unwrap();
    assert_eq!(row["waveformRef"]["trackId"], "track_waveform");
    assert_eq!(row["waveformRef"]["artifactKind"], "waveform");
    assert!(row.get("waveformLevels").is_none());
    assert!(row.get("visibleWaveformSamples").is_none());
}

#[test]
fn controller_non_history_mutation_invalidates_snapshot_undo() {
    let root = test_dir("undo-non-history");
    let audio_path = root.join("song.wav");
    write_test_wav(&audio_path, 8_000, 1, 8_000);
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    state.select_track_state("track_edit");
    let marker_id =
        state.add_marker_to_selected_track_with_duration_state(1.25, 0.5, "Cue", "cue", "cyan");
    assert!(state.can_undo);

    let imported_track_id = state.import_audio_state(audio_path.to_str().unwrap());

    assert!(!imported_track_id.is_empty());
    assert!(!state.can_undo);
    assert!(!state.undo_state());
    assert!(state
        .project
        .tracks
        .iter()
        .any(|track| track.id == imported_track_id));
    assert!(state
        .project
        .markers
        .iter()
        .any(|marker| marker.id == marker_id));
}

#[test]
fn controller_rejects_audio_transform_for_generated_marker_parent_without_audio_artifact() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();

    let track_id = state.add_transform_track_state("track_beats", "waveform.summary", "1", "{}");

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
    let demo_artifact_dir = state.current_artifact_dir().unwrap();
    state.project_path = cxx_qt_lib::QString::from(
        demo_artifact_dir
            .join("show.autolight")
            .to_string_lossy()
            .to_string(),
    );
    let energy_cache_ref = state
        .project
        .tracks
        .iter()
        .find(|track| track.id == "track_drum_energy")
        .unwrap()
        .cache_refs
        .first()
        .cloned()
        .unwrap();
    let energy_entry_path = demo_artifact_dir.join(
        &state
            .project
            .cache_entries
            .iter()
            .find(|entry| entry.id == energy_cache_ref)
            .unwrap()
            .path,
    );
    std::fs::remove_file(energy_entry_path).unwrap();

    let invalid = state.refresh_cache_status_state();

    assert_eq!(invalid, [energy_cache_ref]);
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
fn controller_refresh_cache_status_checks_persisted_artifact_files() {
    let root = test_dir("cache-refresh-files");
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    state.project_path =
        cxx_qt_lib::QString::from(root.join("show.autolight").to_string_lossy().to_string());
    let entry = cache_entry_for_bytes("stem", "dep_drums", b"valid stem", "1", "now").unwrap();
    let artifact_path = root.join(&entry.path);
    std::fs::create_dir_all(artifact_path.parent().unwrap()).unwrap();
    std::fs::write(&artifact_path, b"corrupt stem").unwrap();
    state
        .project
        .cache_entries
        .retain(|entry| entry.id != "cache_drums");
    state.project.cache_entries.push(entry.clone());
    state
        .project
        .tracks
        .iter_mut()
        .find(|track| track.id == "track_drums")
        .unwrap()
        .cache_refs = vec![entry.id.clone()];

    let invalid = state.refresh_cache_status_state();

    assert!(invalid.contains(&entry.id));
    assert_eq!(
        state
            .project
            .tracks
            .iter()
            .find(|track| track.id == "track_drums")
            .unwrap()
            .result_state,
        ResultState::Stale
    );
    assert_eq!(
        state
            .project
            .cache_entries
            .iter()
            .find(|candidate| candidate.id == entry.id)
            .unwrap()
            .validation_status,
        CacheValidationStatus::Invalid
    );
}

#[test]
fn controller_refresh_cache_status_restores_recovered_artifact_validity() {
    let root = test_dir("cache-refresh-recovered");
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    state.project_path =
        cxx_qt_lib::QString::from(root.join("show.autolight").to_string_lossy().to_string());
    let payload = b"valid stem";
    let mut entry = cache_entry_for_bytes("stem", "dep_drums", payload, "1", "now").unwrap();
    entry.validation_status = CacheValidationStatus::Invalid;
    let artifact_path = root.join(&entry.path);
    std::fs::create_dir_all(artifact_path.parent().unwrap()).unwrap();
    std::fs::write(&artifact_path, payload).unwrap();
    state.project.cache_entries.clear();
    state.project.cache_entries.push(entry.clone());
    for track in &mut state.project.tracks {
        track.cache_refs.clear();
    }
    state
        .project
        .tracks
        .iter_mut()
        .find(|track| track.id == "track_drums")
        .unwrap()
        .cache_refs = vec![entry.id.clone()];

    let invalid = state.refresh_cache_status_state();

    assert!(invalid.is_empty());
    assert_eq!(
        state
            .project
            .cache_entries
            .iter()
            .find(|candidate| candidate.id == entry.id)
            .unwrap()
            .validation_status,
        CacheValidationStatus::Valid
    );
}

#[test]
fn controller_rejects_absolute_cache_entry_paths_during_validation() {
    let absolute_path = std::env::current_dir()
        .unwrap()
        .join("autolight-cache")
        .join("entry.bin");
    assert!(!cache_entry_path_is_safe(&absolute_path));
    assert!(!cache_entry_path_is_safe(std::path::Path::new(
        "../cache/entry.bin"
    )));
    assert!(cache_entry_path_is_safe(std::path::Path::new(
        "cache/entry.bin"
    )));
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
    assert!(markers[0]["selected"].as_bool().unwrap_or(false));
    assert!(markers[1]["selected"].as_bool().unwrap_or(false));
    assert!(editable_row["markerSpans"][0]["selected"]
        .as_bool()
        .unwrap_or(false));
}

#[test]
fn controller_edits_selected_markers_roundtrip() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    state.select_track_state("track_edit");

    let marker_id = state
        .add_marker_to_selected_track_with_duration_state(1.25, 0.5, "Blackout", "cue", "cyan");
    assert!(!marker_id.is_empty());
    assert_eq!(
        state.selected_marker_ids.as_slice(),
        std::slice::from_ref(&marker_id)
    );

    assert!(
        state.update_selected_marker_with_duration_state(1.5, 0.75, "Scene", "lighting", "violet",)
    );
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
fn controller_bulk_update_without_marker_selection_updates_track_markers() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    state.select_track_state("track_edit");

    assert!(state.selected_marker_ids.is_empty());
    assert_eq!(
        state.bulk_update_selected_markers_state("Scene", "scene", "blue"),
        2
    );

    let markers: Vec<_> = state
        .project
        .markers
        .iter()
        .filter(|marker| marker.track_id == "track_edit")
        .collect();
    assert!(markers.iter().all(|marker| marker.label == "Scene"));
    assert!(markers
        .iter()
        .all(|marker| marker.metadata["color"] == serde_json::json!("blue")));
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
fn controller_rejects_deriving_editable_track_from_stale_marker_track() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    let initial_track_count = state.project.tracks.len();
    let initial_marker_count = state.project.markers.len();
    state
        .project
        .tracks
        .iter_mut()
        .find(|track| track.id == "track_beats")
        .unwrap()
        .result_state = ResultState::Stale;

    let track_id = state.create_editable_track_from_track_state("track_beats");

    assert!(track_id.is_empty());
    assert_eq!(state.project.tracks.len(), initial_track_count);
    assert_eq!(state.project.markers.len(), initial_marker_count);
    assert!(!state.can_undo);
    assert!(state.last_error.to_string().contains("not complete"));
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
    assert_eq!(
        state.selected_marker_ids.as_slice(),
        std::slice::from_ref(&marker_id)
    );

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
fn controller_marker_undo_preserves_unrelated_state_and_dependent_track_snapshot() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    let generated_track_id = state.add_transform_track_state(
        "track_edit",
        "markers.fixed_interval",
        "1",
        r#"{"duration": 1.0, "interval": 0.5}"#,
    );
    state.run_track_state(&generated_track_id);
    state.select_track_state("track_edit");

    let marker_id = state
        .add_marker_to_selected_track_with_duration_state(1.25, 0.5, "Blackout", "cue", "cyan");
    state.project.name = "Out-of-band Rename".to_string();

    assert!(state.undo_state());
    assert_eq!(state.project.name, "Out-of-band Rename");
    assert!(!state
        .project
        .markers
        .iter()
        .any(|marker| marker.id == marker_id));
    assert_eq!(
        state
            .project
            .tracks
            .iter()
            .find(|track| track.id == generated_track_id)
            .unwrap()
            .result_state,
        ResultState::Complete
    );
    assert_eq!(
        state
            .project
            .markers
            .iter()
            .filter(|marker| marker.track_id == generated_track_id)
            .count(),
        3
    );
}

#[test]
fn controller_save_marks_clean_without_dropping_undo_history() {
    let root = test_dir("save-preserve-undo");
    let project_path = root.join("show.autolight");
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    state.select_track_state("track_edit");
    let marker_id = state
        .add_marker_to_selected_track_with_duration_state(1.25, 0.5, "Blackout", "cue", "cyan");

    assert!(state.save_project_state(project_path.to_str().unwrap()));

    assert!(!state.is_dirty);
    assert!(state.can_undo);
    assert!(state.undo_state());
    assert!(!state
        .project
        .markers
        .iter()
        .any(|marker| marker.id == marker_id));
    assert!(state.is_dirty);
    assert!(state.can_redo);

    assert!(state.redo_state());
    assert!(!state.is_dirty);
    assert!(!state.can_redo);
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
    assert!(!drums["expanded"].as_bool().unwrap_or(true));
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
    assert!(!state.project.audio_assets[0].fingerprint.is_empty());
    assert_eq!(state.project.tracks[0].track_type, TrackType::Source);
    assert_eq!(
        state.project.tracks[0].provenance["asset_id"],
        "asset_rust_0001"
    );
}

#[test]
fn controller_import_audio_updates_native_timeline_scene_snapshot() {
    let root = test_dir("import-audio-scene-snapshot");
    let audio_path = root.join("song.wav");
    write_test_wav(&audio_path, 8_000, 1, 16_000);
    let mut state = AppControllerState::default();

    let track_id = state.import_audio_state(audio_path.to_str().unwrap());
    let parsed: serde_json::Value =
        serde_json::from_str(&state.timeline_scene_snapshot_json_state()).unwrap();
    let imported_track = parsed["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|track| track["trackId"] == track_id)
        .unwrap();

    assert_eq!(parsed["durationSeconds"], 2.0);
    assert_eq!(imported_track["name"], "song");
    assert_eq!(imported_track["trackType"], "source");
    assert!(imported_track["selected"].as_bool().unwrap());
}

#[test]
fn controller_import_audio_accepts_ieee_float_and_extensible_wav_metadata() {
    let root = test_dir("import-common-wav-formats");
    let float_path = root.join("float.wav");
    let extensible_path = root.join("extensible.wav");
    write_test_wav_with_format(&float_path, 8_000, 1, 8_000, 3, 32, None);
    write_test_wav_with_format(
        &extensible_path,
        48_000,
        2,
        48_000,
        0xfffe,
        24,
        Some(WAVE_SUBFORMAT_PCM),
    );
    let mut state = AppControllerState::default();

    let float_track = state.import_audio_state(float_path.to_str().unwrap());
    let extensible_track = state.import_audio_state(extensible_path.to_str().unwrap());

    assert!(!float_track.is_empty());
    assert!(!extensible_track.is_empty());
    assert_eq!(state.project.audio_assets[0].duration, 1.0);
    assert_eq!(state.project.audio_assets[0].sample_rate, 8_000);
    assert_eq!(state.project.audio_assets[0].channels, 1);
    assert_eq!(state.project.audio_assets[1].duration, 1.0);
    assert_eq!(state.project.audio_assets[1].sample_rate, 48_000);
    assert_eq!(state.project.audio_assets[1].channels, 2);
}

#[test]
fn controller_import_audio_rejects_unknown_wav_encoding() {
    let root = test_dir("import-unknown-wav-format");
    let audio_path = root.join("song.wav");
    write_test_wav_with_format(&audio_path, 8_000, 1, 8_000, 6, 8, None);
    let mut state = AppControllerState::default();

    let track_id = state.import_audio_state(audio_path.to_str().unwrap());

    assert!(track_id.is_empty());
    assert!(state
        .last_error
        .to_string()
        .contains("unsupported WAV encoding"));
    assert!(state.project.audio_assets.is_empty());
}

#[test]
fn controller_import_audio_rejects_wav_without_data_chunk() {
    let root = test_dir("import-audio-no-data");
    let audio_path = root.join("song.wav");
    write_test_wav_without_data(&audio_path, 8_000, 1);
    let mut state = AppControllerState::default();

    let track_id = state.import_audio_state(audio_path.to_str().unwrap());

    assert!(track_id.is_empty());
    assert!(state.last_error.to_string().contains("data chunk"));
    assert!(state.project.audio_assets.is_empty());
}

#[test]
fn controller_import_audio_rejects_empty_wav_data_chunk() {
    let root = test_dir("import-audio-empty-data");
    let audio_path = root.join("song.wav");
    write_test_wav(&audio_path, 8_000, 1, 0);
    let mut state = AppControllerState::default();

    let track_id = state.import_audio_state(audio_path.to_str().unwrap());

    assert!(track_id.is_empty());
    assert!(state.last_error.to_string().contains("data chunk"));
    assert!(state.project.audio_assets.is_empty());
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
fn controller_open_project_marks_missing_audio_asset_stale() {
    let root = test_dir("open-missing-audio");
    let audio_path = root.join("song.wav");
    let project_path = root.join("show.autolight");
    write_test_wav(&audio_path, 44_100, 2, 16);
    let mut state = AppControllerState::default();
    let source_id = state.import_audio_state(audio_path.to_str().unwrap());
    let generated_id = state.add_transform_track_state(
        &source_id,
        "markers.fixed_interval",
        "1",
        r#"{"duration": 1.0, "interval": 0.5}"#,
    );
    state.run_track_state(&generated_id);
    assert!(state.save_project_state(project_path.to_str().unwrap()));
    std::fs::remove_file(&audio_path).unwrap();

    let mut opened = AppControllerState::default();
    assert!(opened.open_project_state(project_path.to_str().unwrap()));

    assert!(opened.is_dirty);
    assert_eq!(
        opened.project.audio_assets[0].import_status,
        ImportStatus::Offline
    );
    let source = opened
        .project
        .tracks
        .iter()
        .find(|track| track.id == source_id)
        .unwrap();
    assert_eq!(source.result_state, ResultState::Stale);
    assert!(source.error.contains("input audio asset offline"));
    assert_eq!(
        opened
            .project
            .tracks
            .iter()
            .find(|track| track.id == generated_id)
            .unwrap()
            .result_state,
        ResultState::Stale
    );
}

#[test]
fn controller_open_project_keeps_persisted_offline_audio_clean() {
    let root = test_dir("open-persisted-offline-audio");
    let audio_path = root.join("song.wav");
    let project_path = root.join("show.autolight");
    write_test_wav(&audio_path, 44_100, 2, 16);
    let mut state = AppControllerState::default();
    let source_id = state.import_audio_state(audio_path.to_str().unwrap());
    let generated_id = state.add_transform_track_state(
        &source_id,
        "markers.fixed_interval",
        "1",
        r#"{"duration": 1.0, "interval": 0.5}"#,
    );
    state.run_track_state(&generated_id);
    assert!(state.save_project_state(project_path.to_str().unwrap()));
    std::fs::remove_file(&audio_path).unwrap();
    let mut offline = AppControllerState::default();
    assert!(offline.open_project_state(project_path.to_str().unwrap()));
    assert!(offline.is_dirty);
    assert!(offline.save_project_state(project_path.to_str().unwrap()));

    let mut reopened = AppControllerState::default();
    assert!(reopened.open_project_state(project_path.to_str().unwrap()));

    assert!(!reopened.is_dirty);
    assert_eq!(
        reopened.project.audio_assets[0].import_status,
        ImportStatus::Offline
    );
    assert_eq!(
        reopened
            .project
            .tracks
            .iter()
            .find(|track| track.id == source_id)
            .unwrap()
            .result_state,
        ResultState::Stale
    );
    assert_eq!(
        reopened
            .project
            .tracks
            .iter()
            .find(|track| track.id == generated_id)
            .unwrap()
            .result_state,
        ResultState::Stale
    );
}

#[test]
fn controller_open_project_keeps_persisted_invalid_cache_clean() {
    let root = test_dir("open-persisted-invalid-cache");
    let project_path = root.join("show.autolight");
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    state.project_path = cxx_qt_lib::QString::from(project_path.to_string_lossy().to_string());
    let entry = cache_entry_for_bytes("stem", "dep_drums", b"valid stem", "1", "now").unwrap();
    let artifact_path = root.join(&entry.path);
    std::fs::create_dir_all(artifact_path.parent().unwrap()).unwrap();
    std::fs::write(&artifact_path, b"valid stem").unwrap();
    state
        .project
        .cache_entries
        .retain(|candidate| candidate.id != "cache_drums");
    state.project.cache_entries.push(entry.clone());
    state
        .project
        .tracks
        .iter_mut()
        .find(|track| track.id == "track_drums")
        .unwrap()
        .cache_refs = vec![entry.id.clone()];
    assert!(state.save_project_state(project_path.to_str().unwrap()));
    std::fs::write(&artifact_path, b"corrupt stem").unwrap();
    let mut invalid = AppControllerState::default();
    assert!(invalid.open_project_state(project_path.to_str().unwrap()));
    assert!(invalid.is_dirty);
    assert!(invalid.save_project_state(project_path.to_str().unwrap()));

    let mut reopened = AppControllerState::default();
    assert!(reopened.open_project_state(project_path.to_str().unwrap()));

    assert!(!reopened.is_dirty);
    assert_eq!(
        reopened
            .project
            .cache_entries
            .iter()
            .find(|candidate| candidate.id == entry.id)
            .unwrap()
            .validation_status,
        CacheValidationStatus::Invalid
    );
    assert_eq!(
        reopened
            .project
            .tracks
            .iter()
            .find(|track| track.id == "track_drums")
            .unwrap()
            .result_state,
        ResultState::Stale
    );
}

#[test]
fn controller_open_project_finalizes_persisted_active_jobs() {
    let root = test_dir("open-active-job");
    let project_path = root.join("show.autolight");
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    let track_id = state.add_transform_track_state(
        "track_source",
        "markers.fixed_interval",
        "1",
        r#"{"duration": 1.0, "interval": 0.5}"#,
    );
    let job_id = state
        .job_queue
        .submit(&mut state.project, &track_id)
        .unwrap();
    assert!(state.save_project_state(project_path.to_str().unwrap()));

    let mut opened = AppControllerState::default();
    assert!(opened.open_project_state(project_path.to_str().unwrap()));

    assert!(opened.is_dirty);
    assert!(!opened
        .project
        .job_runs
        .iter()
        .any(|run| matches!(run.state, ResultState::Pending | ResultState::Running)));
    assert_eq!(
        opened
            .project
            .job_runs
            .iter()
            .find(|run| run.id == job_id)
            .unwrap()
            .state,
        ResultState::Stale
    );
    assert_eq!(
        opened
            .project
            .tracks
            .iter()
            .find(|track| track.id == track_id)
            .unwrap()
            .result_state,
        ResultState::Stale
    );
    opened.select_track_state(&track_id);
    assert!(!opened.selected_track_has_running_job);
}

#[test]
fn controller_open_project_validates_cache_artifact_files() {
    let root = test_dir("open-cache-validation");
    let project_path = root.join("show.autolight");
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    state.project_path = cxx_qt_lib::QString::from(project_path.to_string_lossy().to_string());
    let entry = cache_entry_for_bytes("stem", "dep_drums", b"valid stem", "1", "now").unwrap();
    let artifact_path = root.join(&entry.path);
    std::fs::create_dir_all(artifact_path.parent().unwrap()).unwrap();
    std::fs::write(&artifact_path, b"valid stem").unwrap();
    state
        .project
        .cache_entries
        .retain(|candidate| candidate.id != "cache_drums");
    state.project.cache_entries.push(entry.clone());
    state
        .project
        .tracks
        .iter_mut()
        .find(|track| track.id == "track_drums")
        .unwrap()
        .cache_refs = vec![entry.id.clone()];
    assert!(state.save_project_state(project_path.to_str().unwrap()));
    std::fs::write(&artifact_path, b"corrupt stem").unwrap();

    let mut opened = AppControllerState::default();
    assert!(opened.open_project_state(project_path.to_str().unwrap()));

    assert!(opened.is_dirty);
    assert_eq!(
        opened
            .project
            .cache_entries
            .iter()
            .find(|candidate| candidate.id == entry.id)
            .unwrap()
            .validation_status,
        CacheValidationStatus::Invalid
    );
    assert_eq!(
        opened
            .project
            .tracks
            .iter()
            .find(|track| track.id == "track_drums")
            .unwrap()
            .result_state,
        ResultState::Stale
    );
}

#[test]
fn controller_open_project_rejects_invalid_graph_without_replacing_state() {
    let root = test_dir("open-invalid-graph");
    let project_path = root.join("bad.autolight");
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    let original_project_name = state.project_name.to_string();
    let mut invalid_project = serde_json::to_value(&state.project).unwrap();
    invalid_project["tracks"][1]["input_track_ids"] = serde_json::json!(["missing_track"]);
    std::fs::write(
        &project_path,
        serde_json::to_string_pretty(&invalid_project).unwrap(),
    )
    .unwrap();

    assert!(!state.open_project_state(project_path.to_str().unwrap()));

    assert_eq!(state.project_name.to_string(), original_project_name);
    assert!(state
        .last_error
        .to_string()
        .contains("missing input track: missing_track"));
}

#[test]
fn controller_open_project_restores_source_track_when_audio_returns() {
    let root = test_dir("open-restored-audio");
    let audio_path = root.join("song.wav");
    let project_path = root.join("show.autolight");
    write_test_wav(&audio_path, 44_100, 2, 16);
    let mut state = AppControllerState::default();
    let source_id = state.import_audio_state(audio_path.to_str().unwrap());
    let generated_id = state.add_transform_track_state(
        &source_id,
        "markers.fixed_interval",
        "1",
        r#"{"duration": 1.0, "interval": 0.5}"#,
    );
    state.run_track_state(&generated_id);
    assert!(state.save_project_state(project_path.to_str().unwrap()));
    std::fs::remove_file(&audio_path).unwrap();

    let mut offline = AppControllerState::default();
    assert!(offline.open_project_state(project_path.to_str().unwrap()));
    assert_eq!(
        offline.project.audio_assets[0].import_status,
        ImportStatus::Offline
    );
    assert!(offline.save_project_state(project_path.to_str().unwrap()));
    write_test_wav(&audio_path, 44_100, 2, 16);

    let mut reopened = AppControllerState::default();
    assert!(reopened.open_project_state(project_path.to_str().unwrap()));

    assert!(reopened.is_dirty);
    assert_eq!(
        reopened.project.audio_assets[0].import_status,
        ImportStatus::Online
    );
    let source = reopened
        .project
        .tracks
        .iter()
        .find(|track| track.id == source_id)
        .unwrap();
    assert_eq!(source.result_state, ResultState::Complete);
    assert!(source.error.is_empty());
    let generated = reopened
        .project
        .tracks
        .iter()
        .find(|track| track.id == generated_id)
        .unwrap();
    assert_eq!(generated.result_state, ResultState::Complete);
    assert!(generated.error.is_empty());
    reopened.select_track_state(&source_id);
    assert!(reopened.selected_track_can_play);
}

#[test]
fn controller_open_project_relinks_audio_from_project_directory() {
    let source_root = test_dir("open-project-relink-source");
    let project_root = test_dir("open-project-relink-project");
    let original_audio_path = source_root.join("song.wav");
    let project_audio_path = project_root.join("song.wav");
    let project_path = project_root.join("show.autolight");
    write_test_wav(&original_audio_path, 44_100, 2, 16);
    let mut state = AppControllerState::default();
    let source_id = state.import_audio_state(original_audio_path.to_str().unwrap());
    assert!(state.save_project_state(project_path.to_str().unwrap()));
    std::fs::remove_file(&original_audio_path).unwrap();
    write_test_wav(&project_audio_path, 44_100, 2, 16);

    let mut opened = AppControllerState::default();
    assert!(opened.open_project_state(project_path.to_str().unwrap()));

    assert!(opened.is_dirty);
    assert_eq!(
        opened.project.audio_assets[0].path,
        project_audio_path.to_string_lossy().to_string()
    );
    assert_eq!(
        opened.project.audio_assets[0].import_status,
        ImportStatus::Online
    );
    let source = opened
        .project
        .tracks
        .iter()
        .find(|track| track.id == source_id)
        .unwrap();
    assert_eq!(source.result_state, ResultState::Complete);
    assert!(source.error.is_empty());
}

#[test]
fn controller_decodes_windows_file_urls_to_local_paths() {
    let path = path_from_qml("file:///C:/Users/me/My%20Song.wav");

    assert_eq!(path.to_string_lossy(), "C:/Users/me/My Song.wav");
}

#[test]
fn controller_preserves_unc_file_urls_to_network_paths() {
    let path = path_from_qml("file://server/share/My%20Song.wav");

    assert_eq!(path.to_string_lossy(), "//server/share/My Song.wav");
}

#[test]
fn controller_relink_hint_is_constrained_to_basename() {
    let root = test_dir("relink-hint-basename");
    let escaped_dir = root.parent().unwrap().join("escaped");
    std::fs::create_dir_all(&escaped_dir).unwrap();
    let escaped_audio = escaped_dir.join("song.wav");
    write_test_wav(&escaped_audio, 44_100, 2, 16);

    let asset = autolight_core::project::AudioAsset {
        id: "asset".to_string(),
        path: escaped_audio.to_string_lossy().to_string(),
        duration: 1.0,
        sample_rate: 44_100,
        channels: 2,
        fingerprint: String::default(),
        import_status: ImportStatus::Offline,
        relink_hint: "../escaped/song.wav".to_string(),
    };

    assert!(super::project_io::audio_asset_project_dir_relink_path(&asset, Some(&root)).is_none());
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
fn controller_seek_keeps_playhead_inside_timeline_viewport() {
    let root = test_dir("seek-scroll");
    let audio_path = root.join("song.wav");
    write_test_wav(&audio_path, 8_000, 1, 120_000);
    let mut state = AppControllerState::default();
    state.import_audio_state(audio_path.to_str().unwrap());
    assert!(state.play_selected_track_state());
    state.set_timeline_visible_seconds_state(4.0);

    state.seek_playback_state(10.0);

    assert_eq!(state.playback_position_seconds, 10.0);
    assert_eq!(state.timeline_scroll_seconds, 6.0);

    state.nudge_playback_state(-9.0);

    assert_eq!(state.playback_position_seconds, 1.0);
    assert_eq!(state.timeline_scroll_seconds, 1.0);
}

#[test]
fn controller_timeline_duration_includes_marker_extents() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    state.select_track_state("track_edit");

    state.add_marker_to_selected_track_with_duration_state(9.0, 2.5, "Long Cue", "cue", "cyan");

    assert_eq!(state.timeline_duration_seconds, 11.5);
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
fn controller_restore_timeline_view_clamps_scroll_and_clears_navigation_state() {
    let root = test_dir("restore-viewport");
    let audio_path = root.join("song.wav");
    write_test_wav(&audio_path, 8_000, 1, 120_000);
    let mut state = AppControllerState::default();
    state.import_audio_state(audio_path.to_str().unwrap());
    state.sync_playback_position_state(2.0);
    state.begin_timeline_user_navigation_state();
    state.project.ui_state.insert(
        "timeline".to_string(),
        serde_json::json!({
            "pixels_per_second": 144.0,
            "scroll_seconds": 999.0,
            "follow_mode": 1
        }),
    );

    state.restore_timeline_view_state();

    assert_eq!(state.timeline_pixels_per_second, 144.0);
    assert_eq!(state.timeline_visible_seconds, 8.0);
    assert_eq!(state.timeline_scroll_seconds, 7.0);
    assert!(!state.timeline_user_navigation_active);
    assert_eq!(state.timeline_playhead_offscreen_direction, -1);
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
fn controller_snap_uses_visible_generated_timing_rows_only() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();

    state.set_timeline_visible_track_range_state(2, 1);
    assert_eq!(state.snap_timeline_time_state(0.53, false), 0.53);

    state.set_timeline_visible_track_range_state(1, 1);
    assert_eq!(state.snap_timeline_time_state(0.53, false), 0.5);
}

#[test]
fn controller_snap_excludes_stale_generated_timing_rows() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    state
        .project
        .tracks
        .iter_mut()
        .find(|track| track.id == "track_beats")
        .unwrap()
        .result_state = ResultState::Stale;
    state.set_timeline_visible_track_range_state(1, 1);

    assert_eq!(state.snap_timeline_time_state(0.53, false), 0.53);
}

#[test]
fn controller_drag_does_not_snap_marker_to_itself() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    state.select_track_state("track_edit");
    state.set_timeline_visible_track_range_state(2, 1);
    state.toggle_marker_selection_state("marker_edit_2", false);

    assert!(state.move_selected_markers_state(0.03, false));
    let marker = state
        .project
        .markers
        .iter()
        .find(|marker| marker.id == "marker_edit_2")
        .unwrap();
    assert_eq!(marker.timestamp, 0.53);
}

#[test]
fn controller_snapped_single_marker_move_clamps_at_timeline_start() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    state.select_track_state("track_edit");
    state.set_timeline_visible_track_range_state(2, 1);
    state.toggle_marker_selection_state("marker_edit_2", false);

    assert!(state.move_selected_markers_state(-0.75, false));
    let marker = state
        .project
        .markers
        .iter()
        .find(|marker| marker.id == "marker_edit_2")
        .unwrap();
    assert_eq!(marker.timestamp, 0.0);
}

#[test]
fn qml_track_rows_show_track_selection_and_allow_lane_selection() {
    let track_row_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../UI/components/TrackRow.qml"),
    )
    .unwrap();
    let lane_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineLane.qml"),
    )
    .unwrap();

    assert!(track_row_qml.contains(
        "readonly property bool rowSelected: root.appController.selectedTrackId === root.trackId"
    ));
    assert!(track_row_qml.contains("id: selectedTrackStripe"));
    assert!(track_row_qml.contains("visible: root.rowSelected"));
    assert!(track_row_qml.contains("border.width: root.rowSelected ? 2 : 1"));
    assert!(track_row_qml.contains("onClicked: root.trackSelected(root.trackId)"));
    assert!(lane_qml.contains(
        "readonly property bool rowSelected: root.appController.selectedTrackId === root.trackId"
    ));
    assert!(lane_qml.contains("border.width: root.rowSelected ? 2 : 1"));
    assert!(lane_qml.contains("signal clicked(real x)"));
}

#[test]
fn qml_timeline_supports_wheel_scroll_and_anchor_zoom_without_model_reload() {
    let timeline_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineView.qml"),
    )
    .unwrap();
    let scene_header = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.h"),
    )
    .unwrap();
    let scene_cpp = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.cpp"),
    )
    .unwrap();

    assert!(scene_header.contains("void wheelEvent(QWheelEvent* event) override;"));
    assert!(scene_header.contains("void viewportScrollRequested(double pixelDelta);"));
    assert!(scene_header.contains("void viewportZoomRequested(double factor, double anchorX);"));
    assert!(scene_cpp.contains("void TimelineSceneItem::wheelEvent(QWheelEvent* event)"));
    assert!(scene_cpp.contains("event->pixelDelta()"));
    assert!(scene_cpp.contains("event->angleDelta()"));
    assert!(scene_cpp.contains("Qt::ShiftModifier"));
    assert!(scene_cpp.contains("Qt::ControlModifier"));
    assert!(scene_cpp.contains("Qt::MetaModifier"));
    assert!(scene_cpp.contains("emit viewportScrollRequested(scrollDelta);"));
    assert!(scene_cpp.contains(
        "emit viewportZoomRequested(factor, std::max(0.0, event->position().x() - laneOriginX()));"
    ));
    assert!(timeline_qml.contains("onViewportScrollRequested"));
    assert!(
        timeline_qml.contains("timelineRoot.appController.scroll_timeline_by_pixels(pixelDelta)")
    );
    assert!(timeline_qml.contains("onViewportZoomRequested"));
    assert!(timeline_qml.contains(
        "Math.max(0, width - timelineRoot.timelineLabelWidth - timelineRoot.timelineLeftPadding)"
    ));
    assert!(!timeline_qml.contains("WheelHandler {"));
    assert!(!timeline_qml.contains("TimelineNavigationSurface"));
}

#[test]
fn native_timeline_trackpad_scroll_uses_natural_horizontal_direction() {
    let scene_cpp = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.cpp"),
    )
    .unwrap();

    assert!(scene_cpp.contains("scrollDelta = -static_cast<double>(pixelDelta.x());"));
    assert!(scene_cpp.contains("-static_cast<double>(angleDelta.x())"));
    assert!(scene_cpp.contains("kWheelAngleUnitsPerNotch * kScrollPixelsPerNotch"));
    assert!(scene_cpp.contains("scrollDelta = -static_cast<double>(pixelDelta.y());"));
}

#[test]
fn qml_native_timeline_gestures_resume_follow_after_quiet_period() {
    let timeline_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineView.qml"),
    )
    .unwrap();

    assert!(timeline_qml.contains("function extendNativeViewportGesture()"));
    assert!(timeline_qml.contains("timelineRoot.appController.begin_timeline_user_navigation()"));
    assert!(timeline_qml.contains("nativeViewportGestureQuietTimer.restart()"));
    assert!(timeline_qml.contains("id: nativeViewportGestureQuietTimer"));
    assert!(timeline_qml.contains("interval: 220"));
    assert!(timeline_qml.contains("timelineRoot.appController.end_timeline_user_navigation()"));
    assert!(timeline_qml.contains(
        "onViewportScrollRequested: function(pixelDelta) {\n            timelineRoot.extendNativeViewportGesture()"
    ));
    assert!(timeline_qml.contains(
        "onViewportZoomRequested: function(factor, anchorX) {\n            timelineRoot.extendNativeViewportGesture()"
    ));
}

#[test]
fn qml_timeline_exposes_log_zoom_fit_follow_and_playhead_handle() {
    let main_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../UI/Main.qml"),
    )
    .unwrap();
    let timeline_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineView.qml"),
    )
    .unwrap();
    let scene_cpp = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.cpp"),
    )
    .unwrap();
    let adapter_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../UI/AppRuntime.qml"),
    )
    .unwrap();

    assert!(adapter_qml.contains("property real timelineMinPixelsPerSecond"));
    assert!(adapter_qml.contains("property real timelineMaxPixelsPerSecond"));
    assert!(main_qml.contains("function zoomSliderValueForPixels"));
    assert!(main_qml.contains("Math.log"));
    assert!(main_qml.contains("from: 0"));
    assert!(main_qml.contains("to: 1"));
    assert!(main_qml.contains("set_timeline_zoom_for_lane_width"));
    assert!(main_qml.contains("fit_timeline_to_lane_width"));
    assert!(main_qml.contains("followModeSelector"));
    assert!(main_qml.contains("set_timeline_follow_mode"));
    assert!(main_qml.contains("readonly property int markerInspectorWidth"));
    assert!(main_qml.contains("Layout.preferredWidth: root.markerInspectorWidth"));
    assert!(!main_qml.contains("from: 24"));
    assert!(!main_qml.contains("to: 240"));
    assert!(timeline_qml.contains("playbackPositionSeconds:"));
    assert!(timeline_qml.contains("onScrubRequested"));
    assert!(scene_cpp.contains("appendRulerTicks"));
    assert!(scene_cpp.contains("playheadX"));
    assert!(scene_cpp.contains("emit scrubRequested(seconds);"));
}

#[test]
fn qml_app_runtime_smooths_playback_follow_scroll_only_during_follow() {
    let adapter_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../UI/AppRuntime.qml"),
    )
    .unwrap();

    assert!(adapter_qml.contains("import QtQuick"));
    assert!(adapter_qml.contains("readonly property bool smoothTimelineFollow"));
    assert!(adapter_qml.contains(
        "nativeController.playbackIsPlaying && timelineFollowMode !== 0 && !timelineUserNavigationActive"
    ));
    assert!(adapter_qml.contains("Behavior on timelineScrollSeconds"));
    assert!(adapter_qml.contains("enabled: appRuntime.smoothTimelineFollow"));
    assert!(adapter_qml.contains("SmoothedAnimation {"));
    assert!(adapter_qml.contains("velocity: 1.0"));
    assert!(adapter_qml.contains("maximumEasingTime: 80"));
    assert!(!adapter_qml.contains("NumberAnimation {"));
    assert!(!adapter_qml.contains("duration: 120"));
}

#[test]
fn qml_timeline_polish_uses_editor_controls_badges_and_tick_marks() {
    let main_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../UI/Main.qml"),
    )
    .unwrap();
    let playback_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/PlaybackBar.qml"),
    )
    .unwrap();
    let timeline_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineView.qml"),
    )
    .unwrap();
    let scene_cpp = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.cpp"),
    )
    .unwrap();

    assert!(main_qml.contains("id: timelineControlBand"));
    assert!(main_qml.contains("import QtQuick.Controls.Basic as Basic"));
    assert!(main_qml.contains("Basic.Slider {"));
    assert!(main_qml.contains("Basic.ComboBox {"));
    assert!(main_qml.contains("controlBandBackground"));
    assert!(main_qml.contains("id: zoomFitButton"));
    assert!(main_qml.contains("background: Rectangle"));
    assert!(main_qml.contains("text: \"FOLLOW\""));
    assert!(main_qml.contains("text: \"SCROLL\""));

    assert!(timeline_qml.contains("TimelineSceneItem"));
    assert!(scene_cpp.contains("rulerBackground"));
    assert!(scene_cpp.contains("appendRulerTicks"));
    assert!(scene_cpp.contains("selectionStripe"));
    assert!(scene_cpp.contains("selectionOutline"));
    assert!(scene_cpp.contains("laneEven"));
    assert!(scene_cpp.contains("laneOdd"));

    assert!(playback_qml.contains("import QtQuick.Controls.Basic as Basic"));
    assert!(playback_qml.contains("id: playPauseButton"));
    assert!(playback_qml.contains("Basic.Slider {"));
    assert!(playback_qml.contains("id: playbackScrubber"));
}

#[test]
fn qml_timeline_uses_native_scene_waveform_refs_not_qml_waveform_geometry() {
    let timeline_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineView.qml"),
    )
    .unwrap();
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    let parsed: serde_json::Value =
        serde_json::from_str(&state.timeline_scene_snapshot_json_state()).unwrap();
    let scene_cpp = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.cpp"),
    )
    .unwrap();

    assert!(timeline_qml.contains("TimelineSceneItem"));
    assert!(timeline_qml.contains("sceneSnapshotJson:"));
    assert!(parsed["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|track| track["waveformRef"].is_object()));
    assert!(parsed["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|track| track["waveformPreview"]
            .as_array()
            .is_some_and(|samples| !samples.is_empty())));
    assert!(scene_cpp.contains("waveformPreview"));
    assert!(scene_cpp.contains("WaveformSampleSpec"));
    assert!(scene_cpp.contains("waveformCenterY"));
    assert!(!timeline_qml.contains("TimelineWaveformItem"));
    assert!(!timeline_qml.contains("renderTimelineWaveform"));
    assert!(!timeline_qml.contains("geometryJson"));
    assert!(!timeline_qml.contains("waveformLevels"));
    assert!(!timeline_qml.contains("WaveformStrip"));
    assert!(!timeline_qml.contains("Canvas"));
    assert!(!timeline_qml.contains("selectedLevelPair"));
    assert!(!timeline_qml.contains("targetBucketCount"));
}

#[test]
fn qml_timeline_uses_native_scene_analysis_refs_not_qml_analysis_geometry() {
    let timeline_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineView.qml"),
    )
    .unwrap();
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    let parsed: serde_json::Value =
        serde_json::from_str(&state.timeline_scene_snapshot_json_state()).unwrap();
    let scene_cpp = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.cpp"),
    )
    .unwrap();

    assert!(timeline_qml.contains("TimelineSceneItem"));
    assert!(timeline_qml.contains("sceneSnapshotJson:"));
    assert!(parsed["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|track| track["analysisRefs"]
            .as_array()
            .is_some_and(|refs| !refs.is_empty())));
    assert!(parsed["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|track| track["analysisPreviews"]
            .as_array()
            .is_some_and(|previews| !previews.is_empty())));
    assert!(scene_cpp.contains("AnalysisPreviewSpec"));
    assert!(scene_cpp.contains("analysisPreviews"));
    assert!(scene_cpp.contains("appendAnalysisPreview"));
    assert!(!timeline_qml.contains("TimelineAnalysisItem"));
    assert!(!timeline_qml.contains("renderTimelineAnalysis"));
    assert!(!timeline_qml.contains("geometryJson"));
    assert!(!timeline_qml.contains("visibleEnergySamples"));
    assert!(!timeline_qml.contains("visibleHarmonicColorSamples"));
    assert!(!timeline_qml.contains("AnalysisStrip"));
    assert!(!timeline_qml.contains("Canvas"));
}

#[test]
fn native_timeline_viewport_changes_do_not_reparse_scene_snapshot() {
    fn cpp_method_body<'a>(source: &'a str, signature: &str) -> &'a str {
        let signature_start = source
            .find(signature)
            .unwrap_or_else(|| panic!("{signature} is missing"));
        let body_start = signature_start
            + source[signature_start..]
                .find('{')
                .unwrap_or_else(|| panic!("{signature} body is missing"));
        let mut depth = 0_usize;
        for (offset, character) in source[body_start..].char_indices() {
            match character {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let body_end = body_start + offset + character.len_utf8();
                        return &source[body_start..body_end];
                    }
                }
                _ => {}
            }
        }
        panic!("{signature} body is unterminated");
    }

    let scene_cpp = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.cpp"),
    )
    .unwrap();
    let update_paint_node =
        cpp_method_body(&scene_cpp, "QSGNode* TimelineSceneItem::updatePaintNode");
    let viewport_setters = [
        "void TimelineSceneItem::setViewportScrollSeconds",
        "void TimelineSceneItem::setViewportPixelsPerSecond",
        "void TimelineSceneItem::setViewportVisibleSeconds",
        "void TimelineSceneItem::setViewportTrackScrollPixels",
        "void TimelineSceneItem::setPlaybackPositionSeconds",
    ];

    assert!(scene_cpp.contains("m_snapshot->snapshot = parseSnapshot(m_sceneSnapshotJson);"));
    assert!(!update_paint_node.contains("parseSnapshot("));
    for setter in viewport_setters {
        assert!(
            !cpp_method_body(&scene_cpp, setter).contains("parseSnapshot("),
            "{setter} must not parse scene snapshots"
        );
    }
}

#[test]
fn timeline_scene_item_exposes_native_timing_counters_for_manual_profiling() {
    let scene_header = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.h"),
    )
    .unwrap();
    let scene_cpp = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.cpp"),
    )
    .unwrap();

    assert!(scene_header.contains(
        "Q_PROPERTY(qulonglong sceneSnapshotParseCount READ sceneSnapshotParseCount NOTIFY scenePerfCountersChanged)"
    ));
    assert!(scene_header.contains(
        "Q_PROPERTY(qulonglong worstSceneSnapshotParseMicros READ worstSceneSnapshotParseMicros NOTIFY scenePerfCountersChanged)"
    ));
    assert!(scene_header.contains(
        "Q_PROPERTY(qulonglong worstSceneGraphUpdateMicros READ worstSceneGraphUpdateMicros NOTIFY scenePerfCountersChanged)"
    ));
    assert!(scene_header.contains(
        "Q_PROPERTY(qulonglong textTextureCreateCount READ textTextureCreateCount NOTIFY scenePerfCountersChanged)"
    ));
    assert!(scene_header.contains("qulonglong sceneSnapshotParseCount() const;"));
    assert!(scene_header.contains("qulonglong worstSceneSnapshotParseMicros() const;"));
    assert!(scene_header.contains("qulonglong worstSceneGraphUpdateMicros() const;"));
    assert!(scene_header.contains("qulonglong textTextureCreateCount() const;"));
    assert!(scene_header.contains("void scenePerfCountersChanged();"));
    assert!(scene_header.contains("qulonglong m_sceneSnapshotParseCount = 0;"));
    assert!(scene_header.contains("qulonglong m_worstSceneSnapshotParseMicros = 0;"));
    assert!(scene_header.contains("qulonglong m_worstSceneGraphUpdateMicros = 0;"));
    assert!(scene_header.contains("qulonglong m_textTextureCreateCount = 0;"));
    assert!(scene_cpp.contains("QElapsedTimer"));
    assert!(scene_cpp.contains("m_sceneSnapshotParseCount"));
    assert!(scene_cpp.contains("m_textTextureCreateCount"));
}

#[test]
fn qml_timeline_scroll_updates_native_viewport_without_per_frame_geometry_regeneration() {
    let timeline_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineView.qml"),
    )
    .unwrap();
    let scene_cpp = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.cpp"),
    )
    .unwrap();
    let update_paint_node_start = scene_cpp
        .find("QSGNode* TimelineSceneItem::updatePaintNode")
        .unwrap();
    let update_paint_node = &scene_cpp[update_paint_node_start
        ..scene_cpp[update_paint_node_start..]
            .find("void TimelineSceneItem::mousePressEvent")
            .unwrap()
            + update_paint_node_start];

    assert!(timeline_qml.contains("viewportScrollSeconds:"));
    assert!(timeline_qml.contains("viewportPixelsPerSecond:"));
    assert!(timeline_qml.contains("viewportVisibleSeconds:"));
    assert!(!timeline_qml.contains("renderTimelineWaveform("));
    assert!(!timeline_qml.contains("renderTimelineAnalysis("));
    assert!(!timeline_qml.contains("geometryJson"));
    assert!(!update_paint_node.contains("parseSnapshot("));
}

#[test]
fn qml_timeline_uses_native_scene_item_instead_of_reactive_lanes() {
    let timeline_view_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineView.qml"),
    )
    .unwrap();
    let main_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../UI/Main.qml"),
    )
    .unwrap();

    assert!(timeline_view_qml.contains("TimelineSceneItem"));
    assert!(timeline_view_qml.contains("sceneSnapshotJson:"));
    assert!(timeline_view_qml.contains("viewportScrollSeconds:"));
    assert!(timeline_view_qml.contains("viewportPixelsPerSecond:"));
    assert!(!timeline_view_qml.contains("delegate: TrackRow"));
    assert!(!timeline_view_qml.contains("TimelineLane"));
    assert!(!timeline_view_qml.contains("TimelineRuler"));
    assert!(!main_qml.contains("TimelineRuler"));
}

#[test]
fn qml_timeline_scene_hot_path_has_no_geometry_json_or_qml_marker_repeaters() {
    let timeline_view_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineView.qml"),
    )
    .unwrap();
    let rust_app_main = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../autolight-app/src/main.rs"),
    )
    .unwrap();
    let scene_header = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.h"),
    )
    .unwrap();

    assert!(!timeline_view_qml.contains("renderTimelineWaveform("));
    assert!(!timeline_view_qml.contains("renderTimelineAnalysis("));
    assert!(!timeline_view_qml.contains("geometryJson"));
    assert!(!rust_app_main.contains("include_str!(\"../../../UI/components/TimelineLane.qml\")"));
    assert!(!rust_app_main.contains("include_str!(\"../../../UI/components/MarkerBlock.qml\")"));
    assert!(!rust_app_main.contains("include_str!(\"../../../UI/components/TimelineRuler.qml\")"));
    assert!(!rust_app_main
        .contains("include_str!(\"../../../UI/components/TimelineNavigationSurface.qml\")"));
    assert!(!scene_header.contains("Q_PROPERTY(QString geometryJson"));
}

#[test]
fn timeline_scene_snapshot_contains_static_tracks_without_waveform_geometry() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();

    let snapshot_json = state.timeline_scene_snapshot_json_state();
    let parsed: serde_json::Value = serde_json::from_str(&snapshot_json).unwrap();

    assert!(parsed["tracks"].as_array().unwrap().len() >= 3);
    assert!(parsed["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|track| track["waveformRef"].is_object()));
    assert!(parsed["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|track| track["waveformPreview"]
            .as_array()
            .is_some_and(|samples| !samples.is_empty() && samples.len() <= 4_096)));
    assert!(!snapshot_json.contains("waveformLevels"));
    assert!(!snapshot_json.contains("\"rects\""));
    assert!(!snapshot_json.contains("\"bands\""));
}

#[test]
fn timeline_scene_snapshot_preserves_track_tree_metadata_for_native_rendering() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();

    let snapshot_json = state.timeline_scene_snapshot_json_state();
    let parsed: serde_json::Value = serde_json::from_str(&snapshot_json).unwrap();
    let drums = parsed["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|track| track["trackId"] == "track_drums")
        .unwrap();
    let drum_energy = parsed["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|track| track["trackId"] == "track_drum_energy")
        .unwrap();

    assert_eq!(drums["depth"], 1);
    assert!(drums["hasChildren"].as_bool().unwrap());
    assert!(drums["expanded"].as_bool().unwrap());
    assert_eq!(drum_energy["depth"], 2);
    assert!(!drum_energy["hasChildren"].as_bool().unwrap());
}

#[test]
fn controller_viewport_changes_preserve_timeline_scene_snapshot_payload() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    let before_snapshot = state.timeline_scene_snapshot_json_state();

    state.set_timeline_zoom_state(240.0);
    state.set_timeline_scroll_seconds_state(0.75);
    state.set_timeline_visible_seconds_state(1.25);
    state.apply_timeline_follow_state(1.4);
    state.sync_playback_position_state(1.5);

    assert_eq!(state.timeline_scene_snapshot_json_state(), before_snapshot);
}

#[test]
fn timeline_scene_snapshot_preserves_marker_selection_for_native_rendering() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    state.select_track_state("track_edit");
    state.toggle_marker_selection_state("marker_edit_2", false);

    let snapshot_json = state.timeline_scene_snapshot_json_state();
    let parsed: serde_json::Value = serde_json::from_str(&snapshot_json).unwrap();
    let editable_track = parsed["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|track| track["trackId"] == "track_edit")
        .unwrap();
    let selected_marker = editable_track["markers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|marker| marker["markerId"] == "marker_edit_2")
        .unwrap();

    assert!(editable_track["selected"].as_bool().unwrap());
    assert!(selected_marker["selected"].as_bool().unwrap());
    assert!(selected_marker["editable"].as_bool().unwrap());
}

#[test]
fn timeline_scene_item_draws_ruler_markers_selection_and_playhead_without_qml_repeaters() {
    let timeline_view_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineView.qml"),
    )
    .unwrap();
    let scene_header = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.h"),
    )
    .unwrap();
    let scene_cpp = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.cpp"),
    )
    .unwrap();
    let update_paint_node_start = scene_cpp
        .find("QSGNode* TimelineSceneItem::updatePaintNode")
        .unwrap();
    let update_paint_node = &scene_cpp[update_paint_node_start
        ..scene_cpp[update_paint_node_start..]
            .find("void TimelineSceneItem::mousePressEvent")
            .unwrap()
            + update_paint_node_start];

    assert!(timeline_view_qml.contains("TimelineSceneItem"));
    assert!(!timeline_view_qml.contains("Repeater"));
    assert!(!timeline_view_qml.contains("TimelineNavigationSurface"));
    assert!(scene_header.contains("void trackClicked(const QString& trackId);"));
    assert!(scene_header.contains(
        "void markerClicked(const QString& trackId, const QString& markerId, bool additive);"
    ));
    assert!(scene_header.contains("void scrubRequested(double seconds);"));
    assert!(scene_header.contains("void mouseMoveEvent(QMouseEvent* event) override;"));
    assert!(scene_header.contains("void mouseReleaseEvent(QMouseEvent* event) override;"));
    assert!(scene_cpp.contains("appendRulerTicks"));
    assert!(scene_cpp.contains("QString markerId;"));
    assert!(scene_cpp.contains("marker.selected"));
    assert!(scene_cpp.contains("markerRectForTrack"));
    assert!(scene_cpp
        .contains("emit markerClicked(trackId, marker.markerId, additiveSelection(*event));"));
    assert!(scene_cpp.contains("emit scrubRequested(secondsForPosition("));
    assert!(scene_cpp.contains("event->position().x()"));
    assert!(scene_cpp.contains("m_scrubbingRuler"));
    assert!(scene_cpp.contains("selectionStripe"));
    assert!(scene_cpp.contains("playhead"));
    assert!(scene_cpp.contains("kWheelAngleToDeltaFactor"));
    assert!(scene_cpp.contains("kZoomSensitivityBase"));
    assert!(scene_cpp.contains("kWheelAngleUnitsPerNotch"));
    assert!(scene_cpp.contains("kScrollPixelsPerNotch"));
    assert!(scene_cpp.contains("m_snapshot->snapshot = parseSnapshot(m_sceneSnapshotJson);"));
    assert!(!update_paint_node.contains("parseSnapshot("));
}

#[test]
fn qml_native_timeline_keeps_vertical_scroll_and_visible_track_range_current() {
    let timeline_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineView.qml"),
    )
    .unwrap();
    let scene_header = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.h"),
    )
    .unwrap();
    let scene_cpp = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.cpp"),
    )
    .unwrap();

    assert!(timeline_qml.contains("property real trackScrollPixels"));
    assert!(timeline_qml.contains("function updateVisibleTrackRange()"));
    assert!(timeline_qml.contains("onAppControllerChanged: timelineRoot.updateVisibleTrackRange()"));
    assert!(timeline_qml.contains("onTrackCountChanged: timelineRoot.setTrackScrollPixels"));
    assert!(timeline_qml.contains("viewportTrackScrollPixels: timelineRoot.trackScrollPixels"));
    assert!(timeline_qml.contains("onViewportVerticalScrollRequested"));
    assert!(scene_header.contains("Q_PROPERTY(double viewportTrackScrollPixels"));
    assert!(scene_header.contains("void viewportVerticalScrollRequested(double pixelDelta);"));
    assert!(scene_cpp.contains("trackScrollPixels"));
    assert!(scene_cpp.contains("emit viewportVerticalScrollRequested(verticalDelta);"));
}

#[test]
fn qml_timeline_native_scrub_omits_label_width_and_reference_controls_are_guarded() {
    let main_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../UI/Main.qml"),
    )
    .unwrap();
    let timeline_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineView.qml"),
    )
    .unwrap();
    let legacy_lane_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineLane.qml"),
    )
    .unwrap();
    let legacy_analysis_item_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineAnalysisItem.qml"),
    )
    .unwrap();
    let navigation_surface_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineNavigationSurface.qml"),
    )
    .unwrap();
    let track_row_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../UI/components/TrackRow.qml"),
    )
    .unwrap();

    assert!(main_qml.contains("function setTimelineZoomForLaneWidth"));
    assert!(main_qml
        .contains("typeof root.controller.set_timeline_zoom_for_lane_width === \"function\""));
    assert!(main_qml.contains("typeof root.controller.fit_timeline_to_lane_width === \"function\""));
    assert!(main_qml.contains("typeof root.controller.set_timeline_follow_mode === \"function\""));
    assert!(
        main_qml.contains("typeof root.controller.set_timeline_scroll_seconds === \"function\"")
    );
    assert!(main_qml.contains("onMoved: root.setTimelineZoomForLaneWidth("));
    assert!(timeline_qml.contains("var x = (seconds - scene.viewportScrollSeconds)"));
    assert!(timeline_qml.contains("+ timelineRoot.timelineLeftPadding"));
    assert!(!timeline_qml
        .contains("+ timelineRoot.timelineLabelWidth + timelineRoot.timelineLeftPadding"));
    assert!(
        timeline_qml.contains("typeof timelineRoot.appController.set_timeline_visible_track_range")
    );
    assert!(legacy_lane_qml.contains("allowScrub: true"));
    assert!(legacy_lane_qml.contains("onScrubRequested: function(x, laneWidth)"));
    assert!(legacy_analysis_item_qml.contains("Canvas"));
    assert!(legacy_analysis_item_qml.contains("JSON.parse(root.geometryJson)"));
    assert!(legacy_analysis_item_qml.contains("context.fillRect"));
    assert!(navigation_surface_qml.contains("property bool pinchActive"));
    assert!(navigation_surface_qml.contains("property bool scrubActive"));
    assert!(navigation_surface_qml.contains("function finishWheelNavigationQuietPeriod()"));
    assert!(navigation_surface_qml.contains("wheelNavigationQuietTimer.stop()"));
    assert!(track_row_qml.contains("onScrubRequested: function(x, laneWidth)"));
}

#[test]
fn timeline_geometry_item_empty_reason_has_dedicated_notify_signal() {
    let scene_header = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_renderer/scene_graph.h"),
    )
    .unwrap();
    let scene_cpp = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_renderer/scene_graph.cpp"),
    )
    .unwrap();

    assert!(scene_header
        .contains("Q_PROPERTY(QString emptyReason READ emptyReason NOTIFY emptyReasonChanged)"));
    assert!(scene_header.contains("void emptyReasonChanged();"));
    assert!(scene_cpp.contains("emptyReasonForGeometryJson"));
    assert!(scene_cpp.contains("emit emptyReasonChanged();"));
    assert!(!scene_cpp.contains("m_emptyReason = error;"));
}

#[test]
fn timeline_scene_item_draws_track_labels_and_offsets_timeline_content() {
    let timeline_view_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineView.qml"),
    )
    .unwrap();
    let scene_cpp = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.cpp"),
    )
    .unwrap();

    assert!(scene_cpp.contains("constexpr double kLabelWidth = 280.0;"));
    assert!(scene_cpp.contains("QString name;"));
    assert!(scene_cpp.contains(
        "track.name = trackObject.value(QStringLiteral(\"name\")).toString(track.trackId);"
    ));
    assert!(scene_cpp.contains("QSGSimpleTextureNode"));
    assert!(scene_cpp.contains("appendTrackLabel"));
    assert!(scene_cpp.contains("laneOriginX()"));
    assert!(timeline_view_qml.contains(
        "Math.max(0, width - timelineRoot.timelineLabelWidth - timelineRoot.timelineLeftPadding)"
    ));
    assert!(!timeline_view_qml.contains("zoom_timeline_by_factor(factor, anchorX, width)"));
}

#[test]
fn timeline_scene_item_draws_track_tree_indentation_and_disclosure() {
    let timeline_view_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineView.qml"),
    )
    .unwrap();
    let scene_header = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.h"),
    )
    .unwrap();
    let scene_cpp = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.cpp"),
    )
    .unwrap();

    assert!(scene_cpp.contains("int depth = 0;"));
    assert!(scene_cpp.contains("bool hasChildren = false;"));
    assert!(scene_cpp.contains(
        "track.depth = std::max(0, trackObject.value(QStringLiteral(\"depth\")).toInt(0));"
    ));
    assert!(scene_cpp.contains(
        "track.hasChildren = trackObject.value(QStringLiteral(\"hasChildren\")).toBool(false);"
    ));
    assert!(scene_cpp.contains("treeIndentForDepth(track.depth)"));
    assert!(scene_cpp.contains("appendTrackTreeChrome"));
    assert!(scene_cpp.contains("track.expanded ? QStringLiteral(\"v\") : QStringLiteral(\">\")"));
    assert!(
        scene_header.contains("void trackExpansionToggled(const QString& trackId, bool expanded);")
    );
    assert!(scene_cpp.contains("emit trackExpansionToggled(trackId, !track.expanded);"));
    assert!(timeline_view_qml.contains("onTrackExpansionToggled"));
    assert!(timeline_view_qml
        .contains("timelineRoot.appController.set_track_expanded(trackId, expanded)"));
}

#[test]
fn timeline_scene_item_draws_visible_lane_rows_even_without_waveform_or_markers() {
    let scene_cpp = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/timeline_scene/timeline_scene_item.cpp"),
    )
    .unwrap();

    assert!(scene_cpp.contains("appendTrackLaneChrome"));
    assert!(scene_cpp.contains("laneRowBackground"));
    assert!(scene_cpp.contains("laneRowBorder"));
    assert!(scene_cpp.contains("laneCenterGuide"));
    assert!(scene_cpp.contains("appendTimelineGridLines"));
}

#[test]
fn controller_viewport_changes_preserve_timeline_rows_payload() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    let before_rows = state.timeline_rows_json.to_string();

    state.set_timeline_zoom_state(180.0);
    state.set_timeline_scroll_seconds_state(0.5);
    state.set_timeline_visible_seconds_state(3.0);
    state.set_timeline_visible_track_range_state(1, 2);

    assert_eq!(state.timeline_rows_json.to_string(), before_rows);
    assert_eq!(state.timeline_pixels_per_second, 180.0);
    assert_eq!(state.timeline_visible_seconds, 3.0);
}

#[test]
fn controller_waveform_cache_reuses_parsed_artifact_for_viewport_changes() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    let cache_ref = waveform_cache_ref(&state);

    let first =
        state.render_timeline_waveform_state("track_waveform", &cache_ref, waveform_request(0.0));
    let second =
        state.render_timeline_waveform_state("track_waveform", &cache_ref, waveform_request(0.5));

    assert!(first.contains("\"bands\""));
    assert!(second.contains("\"bands\""));
    assert_eq!(state.waveform_cache.parse_count(), 1);
}

#[test]
fn controller_waveform_cache_rejects_provenance_when_artifact_digest_mismatches() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    let cache_ref = waveform_cache_ref(&state);

    state
        .project
        .cache_entries
        .iter_mut()
        .find(|entry| entry.id == cache_ref)
        .unwrap()
        .payload_digest = "changed_digest".to_string();
    let geometry =
        state.render_timeline_waveform_state("track_waveform", &cache_ref, waveform_request(0.0));

    assert_eq!(geometry, "{\"bands\":[]}");
    assert_eq!(state.waveform_cache.parse_count(), 0);
}

#[test]
fn controller_waveform_cache_is_cleared_when_project_is_replaced() {
    let root = test_dir("waveform-cache-clears-on-project-replace");
    let project_path = root.join("empty.autolight");
    ProjectDocument::new("project_empty_test", "Empty")
        .save_path(&project_path)
        .unwrap();
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    let cache_ref = waveform_cache_ref(&state);

    state.render_timeline_waveform_state("track_waveform", &cache_ref, waveform_request(0.0));
    assert_eq!(state.waveform_cache.parse_count(), 1);

    assert!(state.open_project_state(project_path.to_str().unwrap()));
    assert_eq!(state.waveform_cache.parse_count(), 0);

    state.load_demo_project_state();
    let cache_ref = waveform_cache_ref(&state);
    state.render_timeline_waveform_state("track_waveform", &cache_ref, waveform_request(0.0));
    assert_eq!(state.waveform_cache.parse_count(), 1);

    state.clear_project_state();
    assert_eq!(state.waveform_cache.parse_count(), 0);
}

#[test]
fn controller_waveform_render_request_rejects_unknown_cache_ref() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();

    let geometry =
        state.render_timeline_waveform_state("track_waveform", "missing", waveform_request(0.0));

    assert_eq!(geometry, "{\"bands\":[]}");
}

#[test]
fn controller_waveform_render_request_does_not_mark_project_dirty() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    let cache_ref = waveform_cache_ref(&state);
    let before_rows = state.timeline_rows_json.to_string();
    let was_dirty = state.is_dirty;

    state.render_timeline_waveform_state("track_waveform", &cache_ref, waveform_request(0.0));

    assert_eq!(state.timeline_rows_json.to_string(), before_rows);
    assert_eq!(state.is_dirty, was_dirty);
}

#[test]
fn controller_analysis_render_accepts_visible_energy_provenance_array() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    let cache_ref = state
        .project
        .tracks
        .iter()
        .find(|track| track.id == "track_drum_energy")
        .and_then(|track| track.cache_refs.first())
        .cloned()
        .unwrap();

    let geometry = state.render_timeline_analysis_state(
        "track_drum_energy",
        &cache_ref,
        analysis_request(0.0),
    );
    let parsed: Value = serde_json::from_str(&geometry).unwrap();

    assert!(!parsed["bands"][0]["rects"].as_array().unwrap().is_empty());
}

#[test]
fn controller_analysis_render_groups_harmonic_color_provenance_array() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    let entry =
        cache_entry_for_bytes("harmonic-color", "dep_harmonic", b"{}", "1", "test").unwrap();
    let cache_ref = entry.id.clone();
    state.project.cache_entries.push(entry);
    state.project.tracks.push(Track {
        id: "track_harmonic".to_string(),
        track_type: TrackType::Generated,
        name: "Harmonic Color".to_string(),
        input_track_ids: vec!["track_drums".to_string()],
        transform_id: "music.harmonic_color".to_string(),
        transform_params: serde_json::Map::default(),
        transform_version: "1".to_string(),
        output_schema: "artifact.harmonic_color.v1".to_string(),
        dependency_hash: "dep_harmonic".to_string(),
        result_state: ResultState::Complete,
        cache_refs: vec![cache_ref.clone()],
        provenance: serde_json::Map::from_iter([(
            "visible_harmonic_color".to_string(),
            serde_json::json!([
                {"timestamp": 0.0, "color": "#ef4444"},
                {"timestamp": 0.5, "color": "#22c55e"}
            ]),
        )]),
        error: String::default(),
    });

    let geometry =
        state.render_timeline_analysis_state("track_harmonic", &cache_ref, analysis_request(0.0));
    let parsed: Value = serde_json::from_str(&geometry).unwrap();
    let bands = parsed["bands"].as_array().unwrap();

    assert!(bands.iter().any(|band| band["color"] == "#ef4444"));
    assert!(bands.iter().any(|band| band["color"] == "#22c55e"));
}

#[test]
fn controller_native_pixel_scroll_preserves_rows_and_suppresses_follow() {
    let root = test_dir("native-scroll");
    let audio_path = root.join("song.wav");
    write_test_wav(&audio_path, 8_000, 1, 120_000);
    let mut state = AppControllerState::default();
    state.import_audio_state(audio_path.to_str().unwrap());
    state.set_timeline_visible_seconds_state(2.0);
    let before_rows = state.timeline_rows_json.to_string();

    state.scroll_timeline_by_pixels_state(192.0);

    assert_eq!(state.timeline_rows_json.to_string(), before_rows);
    assert_eq!(state.timeline_scroll_seconds, 2.0);
    assert!(state.timeline_user_navigation_active);
}

#[test]
fn controller_viewport_changes_refresh_playhead_offscreen_direction() {
    let root = test_dir("playhead-offscreen-refresh");
    let audio_path = root.join("song.wav");
    write_test_wav(&audio_path, 8_000, 1, 120_000);
    let mut state = AppControllerState::default();
    state.import_audio_state(audio_path.to_str().unwrap());
    assert!(state.play_selected_track_state());
    state.set_timeline_follow_mode_state(0);
    state.set_timeline_visible_seconds_state(2.0);
    state.sync_playback_position_state(5.0);

    state.set_timeline_scroll_seconds_state(4.0);
    assert_eq!(state.timeline_playhead_offscreen_direction, 0);

    state.set_timeline_scroll_seconds_state(6.5);
    assert_eq!(state.timeline_playhead_offscreen_direction, -1);

    state.set_timeline_scroll_seconds_state(1.0);
    assert_eq!(state.timeline_playhead_offscreen_direction, 1);
}

#[test]
fn controller_native_zoom_preserves_anchor_and_rows() {
    let root = test_dir("native-zoom");
    let audio_path = root.join("song.wav");
    write_test_wav(&audio_path, 8_000, 1, 120_000);
    let mut state = AppControllerState::default();
    state.import_audio_state(audio_path.to_str().unwrap());
    state.set_timeline_visible_lane_width_state(480.0);
    state.set_timeline_scroll_seconds_state(1.0);
    let before_rows = state.timeline_rows_json.to_string();

    state.zoom_timeline_by_factor_state(2.0, 120.0, 480.0);

    assert_eq!(state.timeline_rows_json.to_string(), before_rows);
    assert_eq!(state.timeline_pixels_per_second, 192.0);
    assert_eq!(state.timeline_scroll_seconds, 1.625);
    assert_eq!(state.timeline_visible_seconds, 2.5);
}

#[test]
fn controller_scrub_timeline_at_x_seeks_without_edit_history_or_row_rebuild() {
    let root = test_dir("timeline-scrub");
    let audio_path = root.join("song.wav");
    write_test_wav(&audio_path, 8_000, 1, 120_000);
    let mut state = AppControllerState::default();
    state.import_audio_state(audio_path.to_str().unwrap());
    state.reset_history_clean();
    let before_rows = state.timeline_rows_json.to_string();

    let scrubbed_seconds = state.scrub_timeline_at_x_state(288.0, 480.0);

    assert_eq!(scrubbed_seconds, 3.0);
    assert_eq!(state.playback_position_seconds, 3.0);
    assert_eq!(state.timeline_rows_json.to_string(), before_rows);
    assert!(!state.is_dirty);
    assert!(!state.can_undo);
}

#[test]
fn controller_follow_mode_center_scrolls_unless_user_navigation_is_active() {
    let root = test_dir("timeline-follow");
    let audio_path = root.join("song.wav");
    write_test_wav(&audio_path, 8_000, 1, 120_000);
    let mut state = AppControllerState::default();
    state.import_audio_state(audio_path.to_str().unwrap());
    state.set_timeline_visible_seconds_state(4.0);
    state.set_timeline_follow_mode_state(2);

    state.apply_timeline_follow_state(5.0);
    assert_eq!(state.timeline_scroll_seconds, 3.0);

    state.begin_timeline_user_navigation_state();
    state.apply_timeline_follow_state(8.0);
    assert_eq!(state.timeline_scroll_seconds, 3.0);
    assert!(state.timeline_user_navigation_active);

    state.end_timeline_user_navigation_state();
    state.apply_timeline_follow_state(8.0);
    assert_eq!(state.timeline_scroll_seconds, 6.0);
}

#[test]
fn controller_playback_seek_preserves_timeline_rows_payload() {
    let mut state = AppControllerState::default();
    state.load_demo_project_state();
    let before_rows = state.timeline_rows_json.to_string();

    assert!(state.play_selected_track_state());
    state.seek_playback_state(1.75);

    assert_eq!(state.timeline_rows_json.to_string(), before_rows);
    assert_eq!(state.playback_position_seconds, 1.75);
}

#[test]
fn qml_app_runtime_uses_controller_models_and_actions() {
    let main_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../UI/Main.qml"),
    )
    .unwrap();
    let adapter_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../UI/AppRuntime.qml"),
    )
    .unwrap();

    assert!(main_qml.contains("Qt.createComponent(Qt.resolvedUrl(\"AppRuntime.qml\"))"));
    assert!(main_qml.contains("throw new Error"));
    assert!(main_qml.contains("adapter === null"));
    assert!(!main_qml.contains("return null"));
    assert!(main_qml.contains("function createAppRuntime()"));
    assert!(main_qml.contains("readonly property var appRuntime"));
    assert!(adapter_qml.contains("id: appRuntime"));
    assert!(adapter_qml.contains("property var nativeController: AppController {}"));
    assert!(!main_qml.contains("RustAdapter"));
    assert!(!main_qml.contains("rustAdapter"));
    assert!(!adapter_qml.contains("RustAdapter"));
    assert!(!adapter_qml.contains("rustAdapter"));
    assert!(!adapter_qml.contains("rustController"));
    assert!(!main_qml.contains("appRuntimeSource"));
    assert!(!main_qml.contains("rustAdapterSource"));
    assert!(!main_qml.contains("Qt.createQmlObject"));
    assert!(main_qml.contains("WAV audio files (*.wav)"));
    assert!(!main_qml.contains("*.mp3"));

    assert!(adapter_qml.contains("nativeController.transformSpecsJson"));
    assert!(adapter_qml.contains("property var trackRows: []"));
    assert!(adapter_qml.contains("Failed to parse timelineRowsJson"));
    assert!(adapter_qml.contains("Failed to parse transformSpecsJson"));
    assert!(adapter_qml.contains("trackRows = rows"));
    assert!(!adapter_qml.contains("trackModel.append(rows[i])"));
    assert!(adapter_qml.contains("function reloadModels() {\n        reloadSelectionModels()"));
    assert!(adapter_qml.contains("nativeController.selectedTrackId"));
    assert!(adapter_qml.contains("nativeController.addTransformTrack"));
    assert!(adapter_qml.contains("nativeController.runTrack"));
    assert!(adapter_qml.contains("nativeController.pollJobs"));
    assert!(adapter_qml.contains("property var jobPollTimer: Timer"));
    assert!(adapter_qml.contains("nativeController.selectedMarkerIdsJson"));
    assert!(adapter_qml.contains("nativeController.selectedTrackMarkersJson"));
    assert!(adapter_qml.contains("nativeController.markerColorOptionsJson"));
    assert!(adapter_qml.contains("nativeController.addMarkerToSelectedTrackWithDuration"));
    assert!(adapter_qml.contains("nativeController.updateSelectedMarkerWithDuration"));
    assert!(adapter_qml.contains("nativeController.bulkUpdateSelectedMarkers"));
    assert!(adapter_qml.contains("nativeController.toggleMarkerSelection"));
    assert!(adapter_qml.contains("nativeController.createEditableTrackFromTrack"));
    assert!(adapter_qml.contains("nativeController.setTrackExpanded"));
    assert!(adapter_qml.contains("nativeController.undo"));
    assert!(adapter_qml.contains("nativeController.redo"));
    assert!(adapter_qml.contains("nativeController.projectPath"));
    assert!(adapter_qml.contains("nativeController.selectedTrackCanPlay"));
    assert!(adapter_qml.contains("nativeController.openProject"));
    assert!(adapter_qml.contains("nativeController.saveProject"));
    assert!(adapter_qml.contains("nativeController.importAudio"));
    assert!(adapter_qml.contains("nativeController.playSelectedTrack"));
    assert!(adapter_qml.contains("nativeController.playbackSourcePath"));
    assert!(adapter_qml.contains("nativeController.playbackPositionSeconds"));
    assert!(adapter_qml.contains("nativeController.playbackDurationSeconds"));
    assert!(adapter_qml.contains("nativeController.playbackLastError"));
    assert!(adapter_qml.contains("nativeController.playbackVolume"));
    assert!(adapter_qml.contains("nativeController.setPlaybackVolumeValue"));
    assert!(adapter_qml.contains("import QtMultimedia"));
    assert!(adapter_qml.contains("MediaPlayer"));
    assert!(adapter_qml.contains("MediaPlayer.PlayingState"));
    assert!(adapter_qml.contains("AudioOutput"));
    assert!(adapter_qml.contains("mediaPlayer.play()"));
    assert!(adapter_qml.contains("function seekMediaPlayerToSeconds(seconds)"));
    assert!(adapter_qml.contains("mediaPlayer.position = positionMs"));
    assert!(!adapter_qml.contains("mediaPlayer.seek("));
    assert!(adapter_qml.contains("onPositionChanged: function(position)"));
    assert!(adapter_qml.contains("nativeController.syncPlaybackPosition(position / 1000.0)"));
    assert!(adapter_qml.contains(
        "onPositionChanged: function(position) {\n            if (source.toString().length > 0) {\n                nativeController.syncPlaybackPosition(position / 1000.0)\n                reloadViewportState()\n            }\n        }"
    ));
    assert!(!adapter_qml.contains(
        "onPositionChanged: {\n            if (source.toString().length > 0) {\n                nativeController.syncPlaybackPosition(position / 1000.0)"
    ));
    assert!(!adapter_qml.contains(
        "onPositionChanged: { nativeController.seekPlayback(position / 1000.0); reloadModels() }"
    ));
    assert!(adapter_qml.contains("encodeURIComponent(segment)"));
    assert!(adapter_qml.contains("path.replace(/\\\\/g, \"/\")"));
    assert!(adapter_qml.contains("normalizedPath.match(/^[A-Za-z]:\\//)"));
    assert!(adapter_qml.contains("selectedTrackCanRerun = nativeController.selectedTrackCanRerun"));
    assert!(adapter_qml.contains("function select_track(trackId) {\n        nativeController.selectTrack(trackId)\n        reloadSelectionModels()\n        reloadTrackModel()\n        reloadTimelineSceneSnapshot()\n    }"));
    assert!(!adapter_qml.contains("function select_track(trackId) { nativeController.selectTrack(trackId); reloadSelectionModels() }"));
    assert!(!adapter_qml.contains("encodeURI(path)"));
    assert!(adapter_qml.contains("nativeController.timelinePixelsPerSecond"));
    assert!(adapter_qml.contains("nativeController.timelineScrollSeconds"));
    assert!(adapter_qml.contains("nativeController.timelineVisibleSeconds"));
    assert!(
        adapter_qml.contains("timelineDurationSeconds = nativeController.timelineDurationSeconds")
    );
    assert!(adapter_qml.contains("nativeController.setTimelineZoom"));
    assert!(adapter_qml.contains("nativeController.applyTimelineScrollSeconds"));
    assert!(adapter_qml.contains("nativeController.applyTimelineVisibleSeconds"));
    assert!(adapter_qml.contains("nativeController.setTimelineVisibleTrackRange"));
    assert!(adapter_qml.contains(
        "function set_timeline_visible_track_range(firstRow, rowCount) {\n        nativeController.setTimelineVisibleTrackRange(firstRow, rowCount)\n        reloadViewportState()\n    }"
    ));
    assert!(!adapter_qml.contains(
        "function set_timeline_visible_track_range(firstRow, rowCount) { nativeController.setTimelineVisibleTrackRange(firstRow, rowCount); reloadModels() }"
    ));
    assert!(adapter_qml.contains("nativeController.snapTimelineTime"));
    assert!(adapter_qml.contains("function add_fixed_interval_track(trackId, duration, interval) { return add_transform_track"));
    assert!(!adapter_qml.contains("function add_vocals_stem_track"));
    assert!(adapter_qml.contains("transformModel.append"));
    assert!(adapter_qml.contains("function version_at(index)"));

    assert!(adapter_qml.contains("nativeController.timelineSceneSnapshotJson"));
    assert!(main_qml.contains("source: root.pythonReferenceMode ? \"components/LegacyTimelineView.qml\" : \"components/TimelineView.qml\""));
    let timeline_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../UI/components/TimelineView.qml"),
    )
    .unwrap();
    assert!(timeline_qml.contains("TimelineSceneItem"));
    assert!(timeline_qml.contains("sceneSnapshotJson:"));
    assert!(timeline_qml.contains("viewportScrollSeconds:"));
    assert!(!timeline_qml.contains("readonly property var safeTrackRows"));
    assert!(!timeline_qml.contains("delegate: TrackRow"));
    assert!(!timeline_qml.contains("markerSpans: rowData.markerSpans || []"));
    assert!(!timeline_qml.contains("visibleWaveformSamples"));
    assert!(!timeline_qml.contains("model: timelineRows.appController.trackModel"));
}

#[test]
fn qml_app_runtime_refreshes_native_timeline_scene_snapshot_after_model_changes() {
    let adapter_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../UI/AppRuntime.qml"),
    )
    .unwrap();

    assert!(adapter_qml.contains(
        "property string timelineSceneSnapshotJson: nativeController.timelineSceneSnapshotJson"
    ));
    assert!(adapter_qml.contains("function reloadTimelineSceneSnapshot()"));
    assert!(adapter_qml
        .contains("timelineSceneSnapshotJson = nativeController.timelineSceneSnapshotJson"));
    assert!(adapter_qml.contains(
        "function reloadModels() {\n        reloadSelectionModels()\n        reloadViewportState()\n        reloadTrackModel()\n        reloadTimelineSceneSnapshot()\n        reloadTransformModel()"
    ));
    assert!(adapter_qml.contains(
        "function select_track(trackId) {\n        nativeController.selectTrack(trackId)\n        reloadSelectionModels()\n        reloadTrackModel()\n        reloadTimelineSceneSnapshot()\n    }"
    ));
}

#[test]
fn qml_main_initializes_rust_runtime_demo_after_runtime_is_owned() {
    let main_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../UI/Main.qml"),
    )
    .unwrap();
    let adapter_qml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../UI/AppRuntime.qml"),
    )
    .unwrap();

    assert!(main_qml.contains("function initializeRustRuntime()"));
    assert!(main_qml.contains("if (!root.pythonReferenceMode)"));
    assert!(main_qml.contains("root.controller.start_default_project()"));
    assert!(main_qml.contains("Component.onCompleted: root.initializeRustRuntime()"));
    assert!(adapter_qml.contains("function start_default_project()"));
    assert!(!adapter_qml.contains("Component.onCompleted: load_demo_project()"));
}

fn json_array(payload: &str) -> Vec<Value> {
    serde_json::from_str(payload).unwrap()
}

fn waveform_cache_ref(state: &AppControllerState) -> String {
    state
        .project
        .tracks
        .iter()
        .find(|track| track.id == "track_waveform")
        .and_then(|track| track.cache_refs.first())
        .cloned()
        .unwrap()
}

fn waveform_request(scroll_seconds: f64) -> WaveformRenderRequest {
    WaveformRenderRequest {
        scroll_seconds,
        visible_seconds: 1.0,
        pixels_per_second: 96.0,
        width_pixels: 320.0,
        height_pixels: 72.0,
        left_padding_pixels: 24.0,
        device_pixel_ratio: 1.0,
    }
}

fn analysis_request(scroll_seconds: f64) -> AnalysisRenderRequest {
    AnalysisRenderRequest {
        scroll_seconds,
        visible_seconds: 1.0,
        pixels_per_second: 100.0,
        width_pixels: 100.0,
        height_pixels: 16.0,
        left_padding_pixels: 0.0,
    }
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
    write_test_wav_with_format(path, sample_rate, channels, frames, 1, 16, None);
}

fn write_test_wav_with_format(
    path: &std::path::Path,
    sample_rate: u32,
    channels: u16,
    frames: u32,
    audio_format: u16,
    bits_per_sample: u16,
    extensible_subformat: Option<[u8; 16]>,
) {
    use std::io::Write;

    let bytes_per_sample = u32::from(bits_per_sample / 8);
    let data_bytes = frames * u32::from(channels) * bytes_per_sample;
    let byte_rate = sample_rate * u32::from(channels) * bytes_per_sample;
    let block_align = channels * (bits_per_sample / 8);
    let fmt_chunk_size = if extensible_subformat.is_some() {
        40_u32
    } else {
        16_u32
    };
    let riff_size = 4 + (8 + fmt_chunk_size) + (8 + data_bytes);
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(b"RIFF").unwrap();
    file.write_all(&riff_size.to_le_bytes()).unwrap();
    file.write_all(b"WAVE").unwrap();
    file.write_all(b"fmt ").unwrap();
    file.write_all(&fmt_chunk_size.to_le_bytes()).unwrap();
    file.write_all(&audio_format.to_le_bytes()).unwrap();
    file.write_all(&channels.to_le_bytes()).unwrap();
    file.write_all(&sample_rate.to_le_bytes()).unwrap();
    file.write_all(&byte_rate.to_le_bytes()).unwrap();
    file.write_all(&block_align.to_le_bytes()).unwrap();
    file.write_all(&bits_per_sample.to_le_bytes()).unwrap();
    if let Some(subformat) = extensible_subformat {
        file.write_all(&22_u16.to_le_bytes()).unwrap();
        file.write_all(&bits_per_sample.to_le_bytes()).unwrap();
        file.write_all(&0_u32.to_le_bytes()).unwrap();
        file.write_all(&subformat).unwrap();
    }
    file.write_all(b"data").unwrap();
    file.write_all(&data_bytes.to_le_bytes()).unwrap();
    file.write_all(&vec![0_u8; data_bytes as usize]).unwrap();
}

fn write_test_wav_without_data(path: &std::path::Path, sample_rate: u32, channels: u16) {
    use std::io::Write;

    let bits_per_sample = 16_u16;
    let bytes_per_sample = u32::from(bits_per_sample / 8);
    let byte_rate = sample_rate * u32::from(channels) * bytes_per_sample;
    let block_align = channels * (bits_per_sample / 8);
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(b"RIFF").unwrap();
    file.write_all(&36_u32.to_le_bytes()).unwrap();
    file.write_all(b"WAVE").unwrap();
    file.write_all(b"fmt ").unwrap();
    file.write_all(&16_u32.to_le_bytes()).unwrap();
    file.write_all(&1_u16.to_le_bytes()).unwrap();
    file.write_all(&channels.to_le_bytes()).unwrap();
    file.write_all(&sample_rate.to_le_bytes()).unwrap();
    file.write_all(&byte_rate.to_le_bytes()).unwrap();
    file.write_all(&block_align.to_le_bytes()).unwrap();
    file.write_all(&bits_per_sample.to_le_bytes()).unwrap();
}
