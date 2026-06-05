use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::waveform::{WaveformPeakPyramid, WaveformRenderFrame, WaveformRenderRequest};

const MAX_RENDER_RECTS: usize = 2_048;

#[derive(Clone, Copy, Debug)]
pub struct AnalysisRenderRequest {
    pub scroll_seconds: f64,
    pub visible_seconds: f64,
    pub pixels_per_second: f64,
    pub width_pixels: f64,
    pub height_pixels: f64,
    pub left_padding_pixels: f64,
}

#[derive(Debug, Default)]
pub struct WaveformArtifactCache {
    entries: BTreeMap<String, CachedWaveformArtifact>,
    parse_count: usize,
}

#[derive(Debug, Clone)]
pub struct CachedWaveformArtifact {
    pub payload_digest: String,
    pub pyramid: WaveformPeakPyramid,
}

impl WaveformArtifactCache {
    pub fn insert_or_update(
        &mut self,
        cache_ref: &str,
        payload_digest: &str,
        payload: &Value,
    ) -> Result<(), serde_json::Error> {
        if self
            .entries
            .get(cache_ref)
            .is_some_and(|entry| entry.payload_digest == payload_digest)
        {
            return Ok(());
        }
        let pyramid = WaveformPeakPyramid::from_json(payload)?;
        self.entries.insert(
            cache_ref.to_string(),
            CachedWaveformArtifact {
                payload_digest: payload_digest.to_string(),
                pyramid,
            },
        );
        self.parse_count += 1;
        Ok(())
    }

    pub fn render(&self, cache_ref: &str, request: WaveformRenderRequest) -> Option<Value> {
        let entry = self.entries.get(cache_ref)?;
        Some(waveform_frame_to_geometry(entry.pyramid.render(request)))
    }

    pub fn invalidate_missing_or_changed<'a>(
        &mut self,
        valid_entries: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) {
        let valid = valid_entries
            .into_iter()
            .map(|(cache_ref, digest)| (cache_ref.to_string(), digest.to_string()))
            .collect::<BTreeMap<_, _>>();
        self.entries.retain(|cache_ref, entry| {
            valid
                .get(cache_ref)
                .is_some_and(|digest| digest == &entry.payload_digest)
        });
    }

    #[cfg(test)]
    pub fn parse_count(&self) -> usize {
        self.parse_count
    }
}

pub fn waveform_frame_to_geometry(frame: WaveformRenderFrame) -> Value {
    match frame {
        WaveformRenderFrame::PeakColumns(columns) => {
            let peak_rects = columns
                .iter()
                .map(|column| {
                    json!({
                        "x": column.x,
                        "y": column.min_y,
                        "width": column.width,
                        "height": (column.max_y - column.min_y).max(1.0),
                    })
                })
                .collect::<Vec<_>>();
            let rms_rects = columns
                .iter()
                .map(|column| {
                    json!({
                        "x": column.x,
                        "y": column.rms_top_y,
                        "width": column.width,
                        "height": (column.rms_bottom_y - column.rms_top_y).max(1.0),
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "bands": [
                    {"color": "#60a5fa", "rects": peak_rects},
                    {"color": "#bfdbfe", "rects": rms_rects}
                ]
            })
        }
    }
}

pub fn empty_geometry() -> String {
    "{\"bands\":[]}".to_string()
}

pub fn analysis_geometry_from_payload(
    artifact_kind: &str,
    payload: &Value,
    request: AnalysisRenderRequest,
) -> String {
    if !request.width_pixels.is_finite()
        || request.width_pixels <= 0.0
        || !request.height_pixels.is_finite()
        || request.height_pixels <= 0.0
        || !request.pixels_per_second.is_finite()
        || request.pixels_per_second <= 0.0
        || !request.scroll_seconds.is_finite()
        || !request.visible_seconds.is_finite()
        || !request.left_padding_pixels.is_finite()
    {
        return empty_geometry();
    }
    let safe_height = request.height_pixels.max(1.0);
    let scroll_seconds = request.scroll_seconds.max(0.0);
    let visible_samples = visible_analysis_samples(
        &analysis_samples(payload),
        scroll_seconds,
        request.visible_seconds.max(0.0),
        artifact_kind != "energy",
    );
    let limit = analysis_output_limit(request.width_pixels);
    let visible_samples = bounded_analysis_samples(&visible_samples, limit);
    if artifact_kind == "energy" {
        let rects = visible_samples
            .iter()
            .map(|sample| analysis_rect(sample, artifact_kind, request, safe_height))
            .collect::<Vec<_>>();
        return json!({
            "bands": [{
                "color": "#facc15",
                "rects": rects
            }]
        })
        .to_string();
    }
    let mut bands: Vec<(String, Vec<Value>)> = Vec::new();
    for sample in visible_samples {
        let color = sample.color.as_deref().unwrap_or("#93c5fd");
        let rect = analysis_rect(sample, artifact_kind, request, safe_height);
        if let Some((_, rects)) = bands.iter_mut().find(|(band_color, _)| band_color == color) {
            rects.push(rect);
        } else {
            bands.push((color.to_string(), vec![rect]));
        }
    }
    json!({
        "bands": bands
            .into_iter()
            .map(|(color, rects)| json!({"color": color, "rects": rects}))
            .collect::<Vec<_>>()
    })
    .to_string()
}

#[derive(Clone, Debug)]
struct AnalysisSample {
    time: f64,
    intensity: f64,
    color: Option<String>,
}

fn analysis_samples(payload: &Value) -> Vec<AnalysisSample> {
    if let Some(samples) = payload.as_array().or_else(|| {
        payload
            .get("samples")
            .or_else(|| payload.get("frames"))
            .and_then(Value::as_array)
    }) {
        return samples
            .iter()
            .enumerate()
            .map(|(index, sample)| {
                let time = sample
                    .get("time")
                    .or_else(|| sample.get("timestamp"))
                    .and_then(Value::as_f64)
                    .unwrap_or(index as f64);
                let intensity = sample
                    .get("intensity")
                    .or_else(|| sample.get("value"))
                    .or_else(|| sample.get("energy"))
                    .and_then(Value::as_f64)
                    .unwrap_or(1.0);
                AnalysisSample {
                    time,
                    intensity,
                    color: sample
                        .get("color")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                }
            })
            .collect();
    }
    if let Some(bins) = payload.get("bins").and_then(Value::as_array) {
        let sample_rate = payload
            .get("sample_rate")
            .or_else(|| payload.get("sampleRate"))
            .and_then(Value::as_f64)
            .filter(|rate| *rate > 0.0);
        return bins
            .iter()
            .enumerate()
            .filter_map(|(index, bin)| {
                Some(AnalysisSample {
                    time: sample_rate.map_or(index as f64, |rate| index as f64 / rate),
                    intensity: bin.as_f64()?,
                    color: None,
                })
            })
            .collect();
    }
    Vec::new()
}

fn visible_analysis_samples(
    samples: &[AnalysisSample],
    scroll_seconds: f64,
    visible_seconds: f64,
    include_leading_context: bool,
) -> Vec<AnalysisSample> {
    let stop_seconds = scroll_seconds + visible_seconds;
    let mut visible = samples
        .iter()
        .filter(|sample| sample.time >= scroll_seconds && sample.time <= stop_seconds)
        .cloned()
        .collect::<Vec<_>>();
    if include_leading_context {
        if let Some(leading) = samples
            .iter()
            .filter(|sample| sample.time < scroll_seconds)
            .max_by(|left, right| left.time.total_cmp(&right.time))
        {
            let mut leading = leading.clone();
            leading.time = scroll_seconds;
            visible.insert(0, leading);
        }
    }
    visible.sort_by(|left, right| left.time.total_cmp(&right.time));
    visible
}

fn bounded_analysis_samples(samples: &[AnalysisSample], limit: usize) -> Vec<&AnalysisSample> {
    if samples.is_empty() || limit == 0 {
        return Vec::new();
    }
    if samples.len() <= limit {
        return samples.iter().collect();
    }
    if limit == 1 {
        return vec![&samples[0]];
    }
    let last_index = samples.len() - 1;
    (0..limit)
        .map(|index| &samples[index * last_index / (limit - 1)])
        .collect()
}

fn analysis_rect(
    sample: &AnalysisSample,
    artifact_kind: &str,
    request: AnalysisRenderRequest,
    safe_height: f64,
) -> Value {
    let scroll_seconds = request.scroll_seconds.max(0.0);
    let x =
        request.left_padding_pixels + (sample.time - scroll_seconds) * request.pixels_per_second;
    let height = if artifact_kind == "energy" {
        safe_height * sample.intensity.clamp(0.0, 1.0)
    } else {
        safe_height
    };
    json!({
        "x": x.max(request.left_padding_pixels),
        "y": (safe_height - height).max(0.0),
        "width": (request.pixels_per_second / 30.0).clamp(1.0, 8.0),
        "height": height.max(1.0),
    })
}

fn analysis_output_limit(width_pixels: f64) -> usize {
    (width_pixels.ceil().max(0.0) as usize + 1).min(MAX_RENDER_RECTS)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        analysis_geometry_from_payload, empty_geometry, AnalysisRenderRequest,
        WaveformArtifactCache,
    };
    use crate::timeline_renderer::waveform::WaveformRenderRequest;

    #[test]
    fn waveform_cache_reuses_parsed_artifact_for_viewport_changes() {
        let payload = payload();
        let mut cache = WaveformArtifactCache::default();

        cache
            .insert_or_update("cache_waveform", "digest_a", &payload)
            .unwrap();
        cache
            .insert_or_update("cache_waveform", "digest_a", &payload)
            .unwrap();
        let first = cache.render("cache_waveform", request(0.0)).unwrap();
        let second = cache.render("cache_waveform", request(0.5)).unwrap();

        assert_eq!(cache.parse_count(), 1);
        assert!(first.is_object());
        assert!(second.is_object());
    }

    #[test]
    fn waveform_cache_invalidates_when_payload_digest_changes() {
        let payload = payload();
        let mut cache = WaveformArtifactCache::default();

        cache
            .insert_or_update("cache_waveform", "digest_a", &payload)
            .unwrap();
        cache
            .insert_or_update("cache_waveform", "digest_b", &payload)
            .unwrap();

        assert_eq!(cache.parse_count(), 2);
    }

    #[test]
    fn waveform_cache_rejects_unknown_cache_ref() {
        let cache = WaveformArtifactCache::default();

        assert!(cache.render("missing", request(0.0)).is_none());
        assert_eq!(empty_geometry(), "{\"bands\":[]}");
    }

    #[test]
    fn analysis_projection_output_is_bounded_by_visible_width() {
        let payload = json!({
            "samples": (0..1_000)
                .map(|index| json!({"time": index as f64 / 100.0, "energy": 0.5}))
                .collect::<Vec<_>>()
        });

        let geometry = analysis_geometry_from_payload(
            "energy",
            &payload,
            AnalysisRenderRequest {
                scroll_seconds: 0.0,
                visible_seconds: 10.0,
                pixels_per_second: 100.0,
                width_pixels: 32.0,
                height_pixels: 16.0,
                left_padding_pixels: 0.0,
            },
        );
        let parsed: serde_json::Value = serde_json::from_str(&geometry).unwrap();

        assert!(parsed["bands"][0]["rects"].as_array().unwrap().len() <= 33);
    }

    #[test]
    fn analysis_projection_caps_wide_tiles_and_spans_full_width() {
        let payload = json!({
            "samples": (0..10_000)
                .map(|index| json!({"time": index as f64 / 10_000.0, "energy": 0.5}))
                .collect::<Vec<_>>()
        });

        let geometry = analysis_geometry_from_payload(
            "energy",
            &payload,
            AnalysisRenderRequest {
                scroll_seconds: 0.0,
                visible_seconds: 1.0,
                pixels_per_second: 10_000.0,
                width_pixels: 10_000.0,
                height_pixels: 16.0,
                left_padding_pixels: 0.0,
            },
        );
        let parsed: serde_json::Value = serde_json::from_str(&geometry).unwrap();
        let rects = parsed["bands"][0]["rects"].as_array().unwrap();

        assert!(rects.len() <= 2_049);
        assert!(rects
            .last()
            .and_then(|rect| rect["x"].as_f64())
            .is_some_and(|x| x >= 9_990.0));
    }

    #[test]
    fn analysis_projection_accepts_top_level_visible_energy_arrays() {
        let payload = json!([
            {"timestamp": 0.0, "value": 0.2},
            {"timestamp": 0.5, "value": 0.8}
        ]);

        let geometry = analysis_geometry_from_payload(
            "energy",
            &payload,
            AnalysisRenderRequest {
                scroll_seconds: 0.0,
                visible_seconds: 1.0,
                pixels_per_second: 100.0,
                width_pixels: 100.0,
                height_pixels: 16.0,
                left_padding_pixels: 0.0,
            },
        );
        let parsed: serde_json::Value = serde_json::from_str(&geometry).unwrap();

        assert_eq!(parsed["bands"][0]["rects"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn analysis_projection_groups_harmonic_colors_and_preserves_left_context() {
        let payload = json!([
            {"timestamp": 0.0, "color": "#ef4444"},
            {"timestamp": 0.5, "color": "#22c55e"},
            {"timestamp": 1.0, "color": "#3b82f6"}
        ]);

        let geometry = analysis_geometry_from_payload(
            "harmonic-color",
            &payload,
            AnalysisRenderRequest {
                scroll_seconds: 0.75,
                visible_seconds: 0.5,
                pixels_per_second: 100.0,
                width_pixels: 50.0,
                height_pixels: 16.0,
                left_padding_pixels: 0.0,
            },
        );
        let parsed: serde_json::Value = serde_json::from_str(&geometry).unwrap();
        let bands = parsed["bands"].as_array().unwrap();

        assert!(bands.iter().any(|band| band["color"] == "#22c55e"));
        assert!(bands.iter().any(|band| band["color"] == "#3b82f6"));
    }

    fn request(scroll_seconds: f64) -> WaveformRenderRequest {
        WaveformRenderRequest {
            scroll_seconds,
            visible_seconds: 1.0,
            pixels_per_second: 16.0,
            width_pixels: 80.0,
            height_pixels: 40.0,
            left_padding_pixels: 0.0,
            device_pixel_ratio: 1.0,
        }
    }

    fn payload() -> serde_json::Value {
        let samples = (0..16)
            .map(|_| json!({"peak": 0.2, "rms": 0.1, "count": 1, "sum_squares": 0.01}))
            .collect::<Vec<_>>();
        json!({
            "version": 2,
            "sample_rate": 16,
            "duration": 1.0,
            "samples": [],
            "levels": [{
                "bucket_count": 16,
                "samples": samples
            }]
        })
    }
}
