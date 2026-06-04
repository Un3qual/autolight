use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const MAX_WAVEFORM_LOD_BUCKETS: usize = 4_096;
const MAX_VISIBLE_WAVEFORM_SAMPLES: usize = 16;

#[derive(Debug, Error)]
pub enum WaveformError {
    #[error("buckets must be greater than zero")]
    InvalidBucketCount,
    #[error("{0} exceeds u32 range")]
    IntegerOutOfRange(&'static str),
    #[error("cancelled")]
    Cancelled,
    #[error("failed to parse waveform payload: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaveformPayload {
    pub version: u32,
    pub sample_rate: u32,
    pub duration: f64,
    pub samples: Vec<WaveformSample>,
    #[serde(default)]
    pub levels: Vec<WaveformLevel>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaveformLevel {
    pub bucket_count: usize,
    pub samples: Vec<WaveformSample>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaveformSample {
    pub peak: f64,
    pub rms: f64,
    #[serde(default)]
    pub count: u64,
    #[serde(default)]
    pub sum_squares: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisibleWaveform {
    pub duration_seconds: f64,
    pub level_bucket_count: usize,
    pub samples: Vec<VisibleWaveformSample>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisibleWaveformSample {
    pub time: f64,
    pub peak: f64,
    pub rms: f64,
}

pub fn build_waveform_payload_from_mono_samples(
    sample_rate: u32,
    mono_samples: &[f32],
    buckets: usize,
    mut cancel_requested: impl FnMut() -> bool,
) -> Result<WaveformPayload, WaveformError> {
    if buckets == 0 {
        return Err(WaveformError::InvalidBucketCount);
    }
    raise_if_cancelled(&mut cancel_requested)?;

    let frame_count = mono_samples.len();
    let mut levels = Vec::new();
    if frame_count > 0 {
        let base_bucket_count = buckets.min(frame_count);
        let level_bucket_counts = waveform_level_bucket_counts(base_bucket_count, frame_count);
        let finest_bucket_count = *level_bucket_counts
            .last()
            .expect("level count is non-empty for non-empty audio");
        let finest_samples =
            summarize_mono_samples(mono_samples, finest_bucket_count, &mut cancel_requested)?;
        for bucket_count in level_bucket_counts {
            raise_if_cancelled(&mut cancel_requested)?;
            levels.push(WaveformLevel {
                bucket_count,
                samples: derive_waveform_level(&finest_samples, bucket_count),
            });
        }
    }

    Ok(WaveformPayload {
        version: 2,
        sample_rate,
        duration: if sample_rate == 0 {
            0.0
        } else {
            frame_count as f64 / sample_rate as f64
        },
        samples: levels
            .first()
            .map(|level| level.samples.clone())
            .unwrap_or_default(),
        levels,
    })
}

pub fn waveform_level_bucket_counts(base_bucket_count: usize, frame_count: usize) -> Vec<usize> {
    let maximum = MAX_WAVEFORM_LOD_BUCKETS.min(frame_count.max(1));
    let mut counts = vec![base_bucket_count.max(1).min(maximum)];
    while *counts.last().unwrap() < maximum {
        let current = *counts.last().unwrap();
        let next_count = maximum.min(current * 4);
        if next_count == current {
            break;
        }
        counts.push(next_count);
    }
    counts
}

pub fn derive_waveform_level(
    source_samples: &[WaveformSample],
    bucket_count: usize,
) -> Vec<WaveformSample> {
    if bucket_count >= source_samples.len() {
        return source_samples.iter().map(sample_with_energy).collect();
    }
    if bucket_count == 0 {
        return Vec::new();
    }

    let source_ranges = sample_frame_ranges(source_samples);
    let total_frames = source_ranges.last().map_or(0, |(_, stop, _)| *stop);
    if total_frames == 0 {
        return (0..bucket_count)
            .map(|_| WaveformSample {
                peak: 0.0,
                rms: 0.0,
                count: 0,
                sum_squares: 0.0,
            })
            .collect();
    }

    (0..bucket_count)
        .map(|bucket_index| {
            let start = bucket_index * total_frames / bucket_count;
            let stop = (bucket_index + 1) * total_frames / bucket_count;
            let mut peak: f64 = 0.0;
            let mut frame_total = 0_u64;
            let mut square_total = 0.0;
            for (sample_start, sample_stop, sample) in &source_ranges {
                let overlap = (*sample_stop)
                    .min(stop)
                    .saturating_sub((*sample_start).max(start));
                if overlap == 0 {
                    continue;
                }
                let source_frames = (*sample_stop - *sample_start).max(1) as f64;
                peak = peak.max(sample_peak(sample));
                frame_total += overlap as u64;
                square_total += sample_square_total(sample) / source_frames * overlap as f64;
            }
            WaveformSample {
                peak,
                rms: (square_total / frame_total.max(1) as f64).sqrt(),
                count: frame_total,
                sum_squares: square_total,
            }
        })
        .collect()
}

pub fn visible_samples(
    payload: &WaveformPayload,
    scroll_seconds: f64,
    visible_seconds: f64,
    pixels_per_second: f64,
) -> VisibleWaveform {
    let selected_level = select_waveform_level(payload, pixels_per_second);
    let level = selected_level.as_ref();
    let level_bucket_count = normalize_bucket_count(level);
    let duration = payload.duration.max(0.0);
    if duration <= 0.0 || level_bucket_count == 0 {
        return VisibleWaveform {
            duration_seconds: duration,
            level_bucket_count,
            samples: Vec::new(),
        };
    }

    let start_seconds = clamped_scroll_origin(scroll_seconds, duration);
    let stop_seconds = (start_seconds + visible_seconds.max(0.0)).min(duration);
    let start_index = ((start_seconds / duration) * level_bucket_count as f64).floor() as usize;
    let mut stop_index = ((stop_seconds / duration) * level_bucket_count as f64).ceil() as usize;
    if stop_index <= start_index {
        stop_index = (start_index + 1).min(level_bucket_count);
    }
    let stop_index = stop_index.min(level.samples.len());
    let start_index = start_index.min(stop_index);
    let mut samples = (start_index..stop_index)
        .map(|index| visible_sample(level, index, duration, level_bucket_count))
        .collect::<Vec<_>>();
    if samples.len() > MAX_VISIBLE_WAVEFORM_SAMPLES {
        let stride = samples.len().div_ceil(MAX_VISIBLE_WAVEFORM_SAMPLES);
        samples = samples.into_iter().step_by(stride).collect();
    }

    VisibleWaveform {
        duration_seconds: duration,
        level_bucket_count,
        samples,
    }
}

pub fn visible_samples_from_json(
    value: &Value,
    scroll_seconds: f64,
    visible_seconds: f64,
    pixels_per_second: f64,
) -> Result<VisibleWaveform, WaveformError> {
    let payload = waveform_payload_from_json(value)?;
    Ok(visible_samples(
        &payload,
        scroll_seconds,
        visible_seconds,
        pixels_per_second,
    ))
}

fn waveform_payload_from_json(value: &Value) -> Result<WaveformPayload, WaveformError> {
    if value.get("levels").and_then(Value::as_array).is_none() {
        let samples = value
            .get("samples")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new()));
        return Ok(WaveformPayload {
            version: optional_u32(value, "version", 1)?,
            sample_rate: optional_u32(value, "sample_rate", 0)?,
            duration: value.get("duration").and_then(Value::as_f64).unwrap_or(0.0),
            samples: serde_json::from_value(samples)?,
            levels: Vec::new(),
        });
    }
    Ok(serde_json::from_value(value.clone())?)
}

fn optional_u32(value: &Value, key: &'static str, default: u32) -> Result<u32, WaveformError> {
    let Some(raw) = value.get(key).and_then(Value::as_u64) else {
        return Ok(default);
    };
    u32::try_from(raw).map_err(|_| WaveformError::IntegerOutOfRange(key))
}

fn select_waveform_level(
    payload: &WaveformPayload,
    pixels_per_second: f64,
) -> Cow<'_, WaveformLevel> {
    if payload.levels.is_empty() {
        return Cow::Owned(WaveformLevel {
            bucket_count: payload.samples.len(),
            samples: payload.samples.clone(),
        });
    }
    let target_bucket_count = (payload.duration.max(0.0) * pixels_per_second.max(0.0) / 8.0)
        .ceil()
        .max(1.0) as usize;
    payload
        .levels
        .iter()
        .min_by_key(|level| normalize_bucket_count(level).abs_diff(target_bucket_count))
        .map_or_else(
            || {
                Cow::Owned(WaveformLevel {
                    bucket_count: 0,
                    samples: Vec::new(),
                })
            },
            Cow::Borrowed,
        )
}

fn normalize_bucket_count(level: &WaveformLevel) -> usize {
    if level.bucket_count == 0 || level.bucket_count != level.samples.len() {
        level.samples.len()
    } else {
        level.bucket_count
    }
}

fn visible_sample(
    level: &WaveformLevel,
    index: usize,
    duration: f64,
    level_bucket_count: usize,
) -> VisibleWaveformSample {
    let sample = &level.samples[index];
    VisibleWaveformSample {
        time: round6(index as f64 * duration / level_bucket_count.max(1) as f64),
        peak: sample_peak(sample),
        rms: finite_non_negative(sample.rms),
    }
}

fn clamped_scroll_origin(scroll_seconds: f64, duration: f64) -> f64 {
    if scroll_seconds.is_nan() {
        0.0
    } else {
        scroll_seconds.clamp(0.0, duration)
    }
}

fn summarize_mono_samples(
    mono_samples: &[f32],
    bucket_count: usize,
    cancel_requested: &mut impl FnMut() -> bool,
) -> Result<Vec<WaveformSample>, WaveformError> {
    if mono_samples.is_empty() {
        return Ok(Vec::new());
    }
    let bucket_count = bucket_count.min(mono_samples.len());
    let mut summaries = Vec::with_capacity(bucket_count);
    for bucket_index in 0..bucket_count {
        raise_if_cancelled(cancel_requested)?;
        let start = bucket_index * mono_samples.len() / bucket_count;
        let stop = (bucket_index + 1) * mono_samples.len() / bucket_count;
        summaries.push(summarize_slice(&mono_samples[start..stop]));
    }
    Ok(summaries)
}

fn summarize_slice(samples: &[f32]) -> WaveformSample {
    if samples.is_empty() {
        return WaveformSample {
            peak: 0.0,
            rms: 0.0,
            count: 0,
            sum_squares: 0.0,
        };
    }
    let mut peak: f64 = 0.0;
    let mut sum_squares = 0.0;
    for sample in samples {
        let value = f64::from(*sample);
        peak = peak.max(value.abs());
        sum_squares += value * value;
    }
    WaveformSample {
        peak,
        rms: (sum_squares / samples.len() as f64).sqrt(),
        count: samples.len() as u64,
        sum_squares,
    }
}

fn sample_frame_ranges(source_samples: &[WaveformSample]) -> Vec<(usize, usize, &WaveformSample)> {
    let mut ranges = Vec::new();
    let mut cursor = 0_usize;
    for sample in source_samples {
        let count = sample_frame_count(sample);
        if count == 0 {
            continue;
        }
        let stop = cursor + count;
        ranges.push((cursor, stop, sample));
        cursor = stop;
    }
    ranges
}

fn sample_with_energy(sample: &WaveformSample) -> WaveformSample {
    let frame_count = sample_frame_count(sample);
    if frame_count == 0 {
        return WaveformSample {
            peak: 0.0,
            rms: 0.0,
            count: 0,
            sum_squares: 0.0,
        };
    }
    let mut normalized = sample.clone();
    normalized.count = frame_count as u64;
    normalized.sum_squares = sample_square_total(sample);
    normalized
}

fn sample_peak(sample: &WaveformSample) -> f64 {
    finite_non_negative(sample.peak.abs())
}

fn sample_frame_count(sample: &WaveformSample) -> usize {
    sample.count as usize
}

fn sample_square_total(sample: &WaveformSample) -> f64 {
    if sample_frame_count(sample) == 0 {
        return 0.0;
    }
    if sample.sum_squares.is_finite() && sample.sum_squares >= 0.0 {
        return sample.sum_squares;
    }
    let rms = finite_non_negative(sample.rms);
    rms * rms * sample_frame_count(sample) as f64
}

fn finite_non_negative(value: f64) -> f64 {
    if value.is_finite() && value >= 0.0 {
        value
    } else {
        0.0
    }
}

fn raise_if_cancelled(cancel_requested: &mut impl FnMut() -> bool) -> Result<(), WaveformError> {
    if cancel_requested() {
        Err(WaveformError::Cancelled)
    } else {
        Ok(())
    }
}

fn round6(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        build_waveform_payload_from_mono_samples, derive_waveform_level, visible_samples,
        visible_samples_from_json, waveform_level_bucket_counts, WaveformError, WaveformPayload,
        WaveformSample,
    };

    #[test]
    fn waveform_builds_versioned_payload_with_pyramid_levels() {
        let samples = [0.0, 0.25, -0.25, 0.5, -0.5, 0.0, 0.125, -0.125];

        let payload = build_waveform_payload_from_mono_samples(8, &samples, 2, || false).unwrap();

        assert_eq!(payload.version, 2);
        assert_eq!(payload.sample_rate, 8);
        assert_eq!(payload.duration, 1.0);
        assert_eq!(payload.samples.len(), 2);
        assert_eq!(payload.levels[0].bucket_count, 2);
        assert!(payload.levels.last().unwrap().bucket_count > payload.levels[0].bucket_count);
        assert!(payload.samples.iter().all(|sample| sample.peak <= 1.0));
    }

    #[test]
    fn waveform_level_counts_are_bounded_by_maximum_and_frame_count() {
        assert_eq!(waveform_level_bucket_counts(2, 8), [2, 8]);
        assert_eq!(
            *waveform_level_bucket_counts(512, 100_000).last().unwrap(),
            4_096
        );
    }

    #[test]
    fn waveform_lod_derives_weighted_rms_from_frame_counts() {
        let samples = vec![
            WaveformSample {
                peak: 0.1,
                rms: 0.1,
                count: 1,
                sum_squares: 0.01,
            },
            WaveformSample {
                peak: 0.8,
                rms: 0.8,
                count: 9,
                sum_squares: 5.76,
            },
        ];

        let derived = derive_waveform_level(&samples, 1);

        assert_eq!(derived[0].count, 10);
        assert_eq!(derived[0].peak, 0.8);
        assert!((derived[0].rms - (5.77_f64 / 10.0).sqrt()).abs() < 1e-9);
        assert!((derived[0].sum_squares - 5.77).abs() < 1e-9);
    }

    #[test]
    fn waveform_lod_ignores_explicit_zero_count_samples() {
        let samples = vec![sample(1.0, 1.0, 0, 1.0), sample(0.25, 0.25, 8, 0.5)];

        let derived = derive_waveform_level(&samples, 1);

        assert_eq!(derived[0].count, 8);
        assert_eq!(derived[0].peak, 0.25);
        assert!((derived[0].sum_squares - 0.5).abs() < 1e-9);
    }

    #[test]
    fn waveform_lod_preserves_explicit_zero_count_at_source_resolution() {
        let samples = vec![sample(1.0, 1.0, 0, 1.0)];

        let derived = derive_waveform_level(&samples, 1);

        assert_eq!(derived[0], sample(0.0, 0.0, 0, 0.0));
    }

    #[test]
    fn waveform_lod_derives_coarse_buckets_by_frame_coverage() {
        let samples = vec![
            sample(0.1, 0.1, 10, 0.1),
            sample(0.9, 0.6, 90, 32.4),
            sample(0.2, 0.2, 10, 0.4),
        ];

        let derived = derive_waveform_level(&samples, 2);

        assert_eq!(derived[0].count, 55);
        assert_eq!(derived[1].count, 55);
        assert_eq!(derived[0].peak, 0.9);
        assert_eq!(derived[1].peak, 0.9);
        assert!((derived[0].sum_squares - 16.3).abs() < 1e-9);
        assert!((derived[1].sum_squares - 16.6).abs() < 1e-9);
    }

    #[test]
    fn waveform_visible_samples_select_more_detail_when_zoomed_in() {
        let payload = payload_with_levels(8.0, &[8, 64]);

        let overview = visible_samples(&payload, 0.0, 1.0, 12.0);
        let detail = visible_samples(&payload, 0.0, 1.0, 200.0);

        assert_eq!(overview.level_bucket_count, 8);
        assert_eq!(detail.level_bucket_count, 64);
        assert!(detail.samples.len() <= 16);
    }

    #[test]
    fn waveform_visible_samples_scale_partial_windows_and_normalize_bucket_count() {
        let payload = WaveformPayload {
            version: 2,
            sample_rate: 0,
            duration: 10.0,
            samples: Vec::new(),
            levels: vec![super::WaveformLevel {
                bucket_count: 100,
                samples: (0..10)
                    .map(|index| sample(index as f64 / 10.0, 0.05, 1, 0.0))
                    .collect(),
            }],
        };

        let visible = visible_samples(&payload, 9.0, 1.0, 48.0);

        assert_eq!(visible.level_bucket_count, 10);
        assert_eq!(visible.samples.last().unwrap().peak, 0.9);
        assert_eq!(visible.samples.last().unwrap().time, 9.0);
    }

    #[test]
    fn waveform_visible_samples_uses_clamped_scroll_origin_for_stop() {
        let payload = WaveformPayload {
            version: 2,
            sample_rate: 0,
            duration: 10.0,
            samples: Vec::new(),
            levels: vec![super::WaveformLevel {
                bucket_count: 10,
                samples: (0..10)
                    .map(|index| sample(index as f64 / 10.0, 0.05, 1, 0.0))
                    .collect(),
            }],
        };

        let visible = visible_samples(&payload, -5.0, 2.0, 48.0);

        assert_eq!(
            visible
                .samples
                .iter()
                .map(|sample| sample.time)
                .collect::<Vec<_>>(),
            [0.0, 1.0]
        );
    }

    #[test]
    fn waveform_visible_samples_treats_nan_scroll_origin_as_zero() {
        let payload = WaveformPayload {
            version: 2,
            sample_rate: 0,
            duration: 10.0,
            samples: Vec::new(),
            levels: vec![super::WaveformLevel {
                bucket_count: 10,
                samples: (0..10)
                    .map(|index| sample(index as f64 / 10.0, 0.05, 1, 0.0))
                    .collect(),
            }],
        };

        let visible = visible_samples(&payload, f64::NAN, 2.0, 48.0);

        assert_eq!(
            visible
                .samples
                .iter()
                .map(|sample| sample.time)
                .collect::<Vec<_>>(),
            [0.0, 1.0]
        );
    }

    #[test]
    fn waveform_visible_samples_read_legacy_single_sample_payload() {
        let payload = json!({
            "version": 1,
            "duration": 1.0,
            "samples": [{"peak": 0.25, "rms": 0.10}]
        });

        let visible = visible_samples_from_json(&payload, 0.0, 1.0, 96.0).unwrap();

        assert_eq!(visible.level_bucket_count, 1);
        assert_eq!(visible.samples[0].peak, 0.25);
    }

    #[test]
    fn waveform_legacy_payload_rejects_oversized_u32_fields() {
        let payload = json!({
            "version": u64::MAX,
            "sample_rate": u64::MAX,
            "duration": 1.0,
            "samples": [{"peak": 0.25, "rms": 0.10}]
        });

        let error = visible_samples_from_json(&payload, 0.0, 1.0, 96.0).unwrap_err();

        assert!(matches!(error, WaveformError::IntegerOutOfRange("version")));
    }

    #[test]
    fn waveform_build_checks_cancellation_between_buckets() {
        let mut checks = 0;
        let error = build_waveform_payload_from_mono_samples(8, &[0.0, 0.5, -0.5, 0.0], 4, || {
            checks += 1;
            checks > 1
        })
        .unwrap_err();

        assert!(matches!(error, WaveformError::Cancelled));
        assert!(checks > 1);
    }

    fn sample(peak: f64, rms: f64, count: u64, sum_squares: f64) -> WaveformSample {
        WaveformSample {
            peak,
            rms,
            count,
            sum_squares,
        }
    }

    fn payload_with_levels(duration: f64, bucket_counts: &[usize]) -> WaveformPayload {
        WaveformPayload {
            version: 2,
            sample_rate: 0,
            duration,
            samples: Vec::new(),
            levels: bucket_counts
                .iter()
                .copied()
                .map(|bucket_count| super::WaveformLevel {
                    bucket_count,
                    samples: vec![sample(0.1, 0.05, 1, 0.0); bucket_count],
                })
                .collect(),
        }
    }
}
