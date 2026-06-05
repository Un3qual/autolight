use std::path::Path;

use autolight_analysis::waveform::{
    build_waveform_payload_from_mono_samples_with_max_bytes, WaveformError,
    MAX_WAVEFORM_LOD_BUCKETS,
};
use autolight_core::project::JsonObject;
use autolight_core::transforms::TransformRegistry;
use autolight_jobs::queue::{JobRegistry, ProducedMarker, TransformResult, TransformRunError};
use serde_json::Value;

use super::audio::read_wav_mono_samples;
use super::markers::round6;
use super::runnable_transform_ids;

const MAX_FIXED_INTERVAL_MARKERS: usize = 100_000;
const DEFAULT_WAVEFORM_BUCKETS: usize = 4_096;

pub(super) fn job_registry() -> JobRegistry {
    let mut registry = JobRegistry::default();
    for spec in TransformRegistry::with_builtin_transforms()
        .specs()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>()
    {
        let transform_id = spec.id.clone();
        if !runnable_transform_ids().contains(&transform_id.as_str()) {
            continue;
        }
        let register_result = match transform_id.as_str() {
            "markers.fixed_interval" => registry.register(spec, fixed_interval_runner),
            "waveform.summary" => registry.register(spec, waveform_summary_runner),
            _ => unreachable!("runnable transform without a registered runner"),
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
    let duration = fixed_interval_number_param(params, "duration", 0.0)?;
    let interval = fixed_interval_number_param(params, "interval", 1.0)?;
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
    let marker_count = ((duration + 1e-9) / interval).floor() + 1.0;
    if !marker_count.is_finite() || marker_count > MAX_FIXED_INTERVAL_MARKERS as f64 {
        return Err(TransformRunError::Failed(format!(
            "too many markers requested: {}",
            marker_count
        )));
    }
    let marker_count = marker_count as usize;

    let mut markers = Vec::with_capacity(marker_count);
    for index in 0..marker_count {
        if context.cancel_requested() {
            return Err(TransformRunError::Cancelled);
        }
        let current = index as f64 * interval;
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
    }
    context.report_progress(1.0);
    Ok(TransformResult::markers(markers))
}

fn waveform_summary_runner(
    context: &mut autolight_jobs::queue::TransformContext,
    params: &JsonObject,
) -> Result<TransformResult, TransformRunError> {
    if context.cancel_requested() {
        return Err(TransformRunError::Cancelled);
    }
    let audio_path = params
        .get("audio_path")
        .and_then(Value::as_str)
        .ok_or_else(|| TransformRunError::Failed("audio_path is required".to_string()))?;
    let buckets = waveform_bucket_param(params)?;
    let max_bytes = waveform_max_bytes_param(params)?;
    let samples =
        read_wav_mono_samples(Path::new(audio_path)).map_err(TransformRunError::Failed)?;
    context.report_progress(0.1);
    let payload = build_waveform_payload_from_mono_samples_with_max_bytes(
        samples.sample_rate,
        &samples.samples,
        buckets,
        max_bytes,
        || context.cancel_requested(),
    )
    .map_err(waveform_error_to_run_error)?;
    let payload = serde_json::to_vec(&payload)
        .map_err(|error| TransformRunError::Failed(error.to_string()))?;
    context.report_progress(1.0);
    Ok(TransformResult::artifact("waveform", payload))
}

fn fixed_interval_number_param(
    params: &JsonObject,
    key: &str,
    default: f64,
) -> Result<f64, TransformRunError> {
    match params.get(key) {
        Some(value) => value
            .as_f64()
            .ok_or_else(|| TransformRunError::Failed(format!("{key} must be a number"))),
        None => Ok(default),
    }
}

fn waveform_bucket_param(params: &JsonObject) -> Result<usize, TransformRunError> {
    let Some(value) = params.get("buckets") else {
        return Ok(DEFAULT_WAVEFORM_BUCKETS);
    };
    let Some(raw) = value.as_u64() else {
        return Err(TransformRunError::Failed(
            "buckets must be a positive integer".to_string(),
        ));
    };
    let buckets = usize::try_from(raw)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            TransformRunError::Failed("buckets must be a positive integer".to_string())
        })?;
    Ok(buckets.min(MAX_WAVEFORM_LOD_BUCKETS))
}

fn waveform_max_bytes_param(params: &JsonObject) -> Result<Option<usize>, TransformRunError> {
    let Some(value) = params.get("max_bytes").or_else(|| params.get("maxBytes")) else {
        return Ok(None);
    };
    let Some(raw) = value.as_u64() else {
        return Err(TransformRunError::Failed(
            "max_bytes must be a positive integer".to_string(),
        ));
    };
    let max_bytes = usize::try_from(raw).map_err(|_| {
        TransformRunError::Failed("max_bytes is too large for this platform".to_string())
    })?;
    if max_bytes == 0 {
        return Err(TransformRunError::Failed(
            "max_bytes must be a positive integer".to_string(),
        ));
    }
    Ok(Some(max_bytes))
}

fn waveform_error_to_run_error(error: WaveformError) -> TransformRunError {
    match error {
        WaveformError::Cancelled => TransformRunError::Cancelled,
        error => TransformRunError::Failed(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use serde_json::json;

    use super::super::audio::write_silent_wav;
    use super::{
        job_registry, waveform_bucket_param, waveform_max_bytes_param, DEFAULT_WAVEFORM_BUCKETS,
    };
    use autolight_analysis::waveform::{WaveformPayload, WaveformSample, MAX_WAVEFORM_LOD_BUCKETS};
    use autolight_core::project::{
        AudioAsset, ImportStatus, JsonObject, ProjectDocument, ResultState, Track, TrackType,
    };
    use autolight_jobs::queue::LocalJobQueue;

    #[test]
    fn waveform_bucket_param_defaults_to_high_resolution_lod_base() {
        let params = JsonObject::new();

        assert_eq!(waveform_bucket_param(&params).unwrap(), 4_096);
        assert_eq!(DEFAULT_WAVEFORM_BUCKETS, 4_096);
    }

    #[test]
    fn waveform_bucket_param_clamps_to_high_resolution_lod_ceiling() {
        let mut params = JsonObject::new();
        params.insert("buckets".to_string(), json!(usize::MAX));

        assert_eq!(
            waveform_bucket_param(&params).unwrap(),
            MAX_WAVEFORM_LOD_BUCKETS
        );
    }

    #[test]
    fn waveform_bucket_param_keeps_default_when_only_max_bytes_is_present() {
        let mut params = JsonObject::new();
        params.insert("max_bytes".to_string(), json!(1_024));

        assert_eq!(
            waveform_bucket_param(&params).unwrap(),
            DEFAULT_WAVEFORM_BUCKETS
        );
    }

    #[test]
    fn waveform_bucket_param_keeps_lod_clamp_separate_from_memory_budget() {
        let mut params = JsonObject::new();
        params.insert("buckets".to_string(), json!(MAX_WAVEFORM_LOD_BUCKETS * 2));
        params.insert("max_bytes".to_string(), json!(1_024));

        assert_eq!(
            waveform_bucket_param(&params).unwrap(),
            MAX_WAVEFORM_LOD_BUCKETS
        );
    }

    #[test]
    fn waveform_max_bytes_param_defaults_to_none() {
        let params = JsonObject::new();

        assert_eq!(waveform_max_bytes_param(&params).unwrap(), None);
    }

    #[test]
    fn waveform_max_bytes_param_accepts_positive_unsigned_integer() {
        let mut params = JsonObject::new();
        params.insert("max_bytes".to_string(), json!(1_048_576));

        assert_eq!(waveform_max_bytes_param(&params).unwrap(), Some(1_048_576));
    }

    #[test]
    fn waveform_max_bytes_param_accepts_legacy_camel_case_alias() {
        let mut params = JsonObject::new();
        params.insert("maxBytes".to_string(), json!(1_048_576));

        assert_eq!(waveform_max_bytes_param(&params).unwrap(), Some(1_048_576));
    }

    #[test]
    fn waveform_max_bytes_param_rejects_zero_and_non_integer() {
        for value in [json!(0), json!(-1), json!(1.5), json!("1024")] {
            let mut params = JsonObject::new();
            params.insert("max_bytes".to_string(), value);

            assert!(waveform_max_bytes_param(&params).is_err());
        }
    }

    #[test]
    fn waveform_max_bytes_param_has_distinct_platform_overflow_error() {
        let source = include_str!("jobs.rs");
        let message = ["max_bytes", " is too large", " for this platform"].concat();

        assert!(source.contains("usize::try_from(raw).map_err"));
        assert!(source.contains(&message));
    }

    #[test]
    fn waveform_summary_runner_applies_max_bytes_to_artifact_payload() {
        let root = test_dir("waveform-summary-budgeted-artifact");
        let audio_path = root.join("source.wav");
        let artifact_dir = root.join("artifacts");
        std::fs::create_dir_all(&artifact_dir).unwrap();
        write_silent_wav(&audio_path, 1_024, 1, 1_024).unwrap();
        let max_bytes = std::mem::size_of::<WaveformSample>() * 64;
        let mut project = project_with_waveform_track(&audio_path, 128, max_bytes);
        let mut queue = LocalJobQueue::with_clock(job_registry(), deterministic_clock());

        let job_id = queue.submit(&mut project, "track_waveform").unwrap();
        assert_eq!(
            queue
                .run_next_with_artifact_dir(&mut project, Some(&artifact_dir))
                .unwrap(),
            Some(job_id)
        );

        let cache_entry = project
            .cache_entries
            .iter()
            .find(|entry| entry.artifact_kind == "waveform")
            .unwrap();
        let payload_bytes = std::fs::read(artifact_dir.join(&cache_entry.path)).unwrap();
        let payload: WaveformPayload = serde_json::from_slice(&payload_bytes).unwrap();

        assert_eq!(
            payload
                .levels
                .iter()
                .map(|level| level.bucket_count)
                .collect::<Vec<_>>(),
            [32]
        );
        assert_eq!(payload.samples.len(), 32);
        assert_eq!(payload_sample_count(&payload), 64);
    }

    fn project_with_waveform_track(
        audio_path: &Path,
        buckets: usize,
        max_bytes: usize,
    ) -> ProjectDocument {
        let mut project = ProjectDocument::new("project_1", "Waveform Budget Test");
        project.audio_assets.push(AudioAsset {
            id: "asset_source".to_string(),
            path: audio_path.to_string_lossy().into_owned(),
            duration: 1.0,
            sample_rate: 1_024,
            channels: 1,
            fingerprint: "fingerprint".to_string(),
            import_status: ImportStatus::Online,
            relink_hint: String::default(),
        });
        project.tracks.extend([
            Track {
                id: "track_source".to_string(),
                track_type: TrackType::Source,
                name: "Source".to_string(),
                input_track_ids: Vec::default(),
                transform_id: String::default(),
                transform_params: JsonObject::default(),
                transform_version: String::default(),
                output_schema: String::default(),
                dependency_hash: String::default(),
                result_state: ResultState::Complete,
                cache_refs: Vec::default(),
                provenance: object(json!({"asset_id": "asset_source"})),
                error: String::default(),
            },
            Track {
                id: "track_waveform".to_string(),
                track_type: TrackType::Generated,
                name: "Waveform".to_string(),
                input_track_ids: vec!["track_source".to_string()],
                transform_id: "waveform.summary".to_string(),
                transform_params: object(json!({
                    "audio_path": audio_path.to_string_lossy(),
                    "buckets": buckets,
                    "max_bytes": max_bytes,
                })),
                transform_version: "1".to_string(),
                output_schema: "artifact.waveform.v1".to_string(),
                dependency_hash: String::default(),
                result_state: ResultState::Complete,
                cache_refs: Vec::default(),
                provenance: JsonObject::default(),
                error: String::default(),
            },
        ]);
        project
    }

    fn payload_sample_count(payload: &WaveformPayload) -> usize {
        payload
            .levels
            .iter()
            .map(|level| level.samples.len())
            .sum::<usize>()
            .saturating_add(payload.samples.len())
    }

    fn deterministic_clock() -> impl FnMut() -> String {
        let mut tick = 0_u64;
        move || {
            tick += 1;
            format!("2026-06-05T00:00:{tick:02}Z")
        }
    }

    fn test_dir(name: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "autolight-qt-jobs-{name}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn object(value: serde_json::Value) -> JsonObject {
        value.as_object().cloned().unwrap()
    }
}
