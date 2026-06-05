use std::path::Path;

use autolight_analysis::waveform::{
    build_waveform_payload_from_mono_samples, WaveformError, MAX_WAVEFORM_LOD_BUCKETS,
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
    let samples =
        read_wav_mono_samples(Path::new(audio_path)).map_err(TransformRunError::Failed)?;
    context.report_progress(0.1);
    let payload = build_waveform_payload_from_mono_samples(
        samples.sample_rate,
        &samples.samples,
        buckets,
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

fn waveform_error_to_run_error(error: WaveformError) -> TransformRunError {
    match error {
        WaveformError::Cancelled => TransformRunError::Cancelled,
        error => TransformRunError::Failed(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{waveform_bucket_param, DEFAULT_WAVEFORM_BUCKETS};
    use autolight_analysis::waveform::MAX_WAVEFORM_LOD_BUCKETS;
    use autolight_core::project::JsonObject;

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
}
