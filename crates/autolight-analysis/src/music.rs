use autolight_core::project::JsonObject;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

pub const DEFAULT_MAX_FRAMES: usize = 2_048;
pub const DEFAULT_MAX_MARKERS: usize = 2_048;

#[derive(Debug, Error)]
pub enum MusicError {
    #[error("{0} must be a positive integer")]
    InvalidPositiveInteger(&'static str),
    #[error("cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MusicAnalysisResult {
    pub kind: String,
    pub payload: Value,
    #[serde(default)]
    pub markers: Vec<AnalysisMarker>,
    #[serde(default)]
    pub frames: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisMarker {
    pub timestamp: f64,
    pub label: String,
    pub category: String,
    pub confidence: Option<f64>,
    #[serde(default)]
    pub metadata: JsonObject,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisibleAnalysisFrames {
    pub kind: String,
    pub frames: Vec<Value>,
}

pub fn analyze_rhythm_fixture(
    duration: f64,
    beat_times: &[f64],
    max_markers: usize,
    mut cancel_requested: impl FnMut() -> bool,
) -> Result<MusicAnalysisResult, MusicError> {
    validate_positive(max_markers, "max_markers")?;
    raise_if_cancelled(&mut cancel_requested)?;
    let beat_times = beat_times
        .iter()
        .copied()
        .filter(|timestamp| timestamp.is_finite() && *timestamp >= 0.0)
        .take(max_markers)
        .map(round6)
        .collect::<Vec<_>>();
    let tempo = estimate_tempo(&beat_times);
    let markers = beat_times
        .iter()
        .enumerate()
        .map(|(index, timestamp)| {
            let mut metadata = JsonObject::new();
            metadata.insert("beat_index".to_string(), json!(index));
            metadata.insert("tempo".to_string(), json!(tempo));
            metadata.insert("beat_strength".to_string(), json!(1.0));
            metadata.insert("source".to_string(), json!("rust.fixture.beat_grid"));
            AnalysisMarker {
                timestamp: *timestamp,
                label: "Beat".to_string(),
                category: "beat".to_string(),
                confidence: Some(1.0),
                metadata,
            }
        })
        .collect::<Vec<_>>();
    Ok(MusicAnalysisResult {
        kind: "beat-grid".to_string(),
        payload: json!({
            "version": 1,
            "kind": "beat-grid",
            "duration": finite_non_negative(duration),
            "tempo": tempo,
            "beat_times": beat_times,
            "settings": {"max_markers": max_markers},
        }),
        markers,
        frames: Vec::new(),
    })
}

pub fn analyze_energy_fixture(
    duration: f64,
    values: &[f64],
    max_frames: usize,
    max_markers: usize,
    mut cancel_requested: impl FnMut() -> bool,
) -> Result<MusicAnalysisResult, MusicError> {
    validate_positive(max_frames, "max_frames")?;
    validate_positive(max_markers, "max_markers")?;
    raise_if_cancelled(&mut cancel_requested)?;
    let normalized = normalize(values);
    raise_if_cancelled(&mut cancel_requested)?;
    let frames = decimated_frames(
        &normalized,
        finite_non_negative(duration),
        max_frames,
        "intensity",
    );
    let markers = energy_markers(&frames, max_markers);
    Ok(MusicAnalysisResult {
        kind: "energy".to_string(),
        payload: json!({
            "version": 1,
            "kind": "energy",
            "duration": finite_non_negative(duration),
            "frames": frames,
            "settings": {"max_frames": max_frames, "max_markers": max_markers},
        }),
        markers,
        frames,
    })
}

pub fn analyze_harmonic_fixture(
    duration: f64,
    chroma_vectors: &[[f64; 12]],
    max_frames: usize,
    max_markers: usize,
    mut cancel_requested: impl FnMut() -> bool,
) -> Result<MusicAnalysisResult, MusicError> {
    validate_positive(max_frames, "max_frames")?;
    validate_positive(max_markers, "max_markers")?;
    raise_if_cancelled(&mut cancel_requested)?;
    let frames = chroma_frames(chroma_vectors, finite_non_negative(duration), max_frames);
    raise_if_cancelled(&mut cancel_requested)?;
    let markers = harmonic_change_markers(&frames, max_markers);
    Ok(MusicAnalysisResult {
        kind: "harmonic-color".to_string(),
        payload: json!({
            "version": 1,
            "kind": "harmonic-color",
            "duration": finite_non_negative(duration),
            "frames": frames,
            "settings": {"max_frames": max_frames, "max_markers": max_markers},
        }),
        markers,
        frames,
    })
}

pub fn visible_analysis_frames(
    payload: &Value,
    scroll_seconds: f64,
    visible_seconds: f64,
    max_frames: usize,
) -> Result<VisibleAnalysisFrames, MusicError> {
    validate_positive(max_frames, "max_frames")?;
    let kind = payload
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let frames = payload
        .get("frames")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let start = scroll_seconds.max(0.0);
    let stop = start + visible_seconds.max(0.0);
    let mut valid_frames = frames
        .into_iter()
        .filter_map(|frame| frame_time(&frame).map(|time| (time, frame)))
        .filter(|(time, _)| time.is_finite())
        .collect::<Vec<_>>();
    valid_frames.sort_by(|left, right| left.0.total_cmp(&right.0));

    let mut visible = Vec::new();
    if kind == "harmonic-color" {
        if let Some((_, frame)) = valid_frames.iter().rev().find(|(time, _)| *time < start) {
            let mut frame = frame.clone();
            if let Some(object) = frame.as_object_mut() {
                object.insert("time".to_string(), json!(round6(start)));
            }
            visible.push(frame);
        }
    }
    visible.extend(
        valid_frames
            .into_iter()
            .filter(|(time, _)| *time >= start && *time <= stop)
            .map(|(_, frame)| frame),
    );
    if visible.len() > max_frames {
        let stride = visible.len().div_ceil(max_frames);
        visible = visible
            .into_iter()
            .step_by(stride)
            .take(max_frames)
            .collect();
    }

    Ok(VisibleAnalysisFrames {
        kind,
        frames: visible,
    })
}

fn decimated_frames(
    values: &[f64],
    duration: f64,
    max_frames: usize,
    value_key: &str,
) -> Vec<Value> {
    if values.is_empty() {
        return Vec::new();
    }
    let stride = values.len().div_ceil(max_frames).max(1);
    values
        .iter()
        .enumerate()
        .step_by(stride)
        .take(max_frames)
        .map(|(index, value)| {
            let time = if values.len() <= 1 {
                0.0
            } else {
                index as f64 * duration / (values.len() - 1) as f64
            };
            json!({"time": round6(time), value_key: round6(*value)})
        })
        .collect()
}

fn energy_markers(frames: &[Value], max_markers: usize) -> Vec<AnalysisMarker> {
    let intensities = frames
        .iter()
        .filter_map(|frame| frame.get("intensity").and_then(Value::as_f64))
        .collect::<Vec<_>>();
    if intensities.is_empty() {
        return Vec::new();
    }
    let mean = intensities.iter().sum::<f64>() / intensities.len() as f64;
    let variance = intensities
        .iter()
        .map(|value| {
            let delta = value - mean;
            delta * delta
        })
        .sum::<f64>()
        / intensities.len() as f64;
    let threshold = 0.65_f64.max(mean + variance.sqrt());

    intensities
        .iter()
        .enumerate()
        .filter(|(index, value)| {
            if *index == 0 || *index + 1 >= intensities.len() {
                return false;
            }
            **value >= threshold
                && **value >= intensities[index - 1]
                && **value >= intensities[index + 1]
        })
        .take(max_markers)
        .map(|(index, value)| {
            let timestamp = frames[index]
                .get("time")
                .and_then(Value::as_f64)
                .unwrap_or_default();
            let mut metadata = JsonObject::new();
            metadata.insert("intensity".to_string(), json!(round6(*value)));
            metadata.insert("source".to_string(), json!("rms_onset_intensity"));
            AnalysisMarker {
                timestamp,
                label: "Energy Peak".to_string(),
                category: "energy_peak".to_string(),
                confidence: Some(round6(*value)),
                metadata,
            }
        })
        .collect()
}

fn chroma_frames(chroma_vectors: &[[f64; 12]], duration: f64, max_frames: usize) -> Vec<Value> {
    if chroma_vectors.is_empty() {
        return Vec::new();
    }
    let stride = chroma_vectors.len().div_ceil(max_frames).max(1);
    chroma_vectors
        .iter()
        .enumerate()
        .step_by(stride)
        .take(max_frames)
        .map(|(index, vector)| {
            let normalized = normalize(vector);
            let energy = vector.iter().copied().filter(|value| *value > 0.0).sum::<f64>();
            let dominant = if energy > 1e-6 {
                normalized
                    .iter()
                    .enumerate()
                    .max_by(|left, right| left.1.total_cmp(right.1))
                    .map(|(index, _)| index as i64)
                    .unwrap_or(-1)
            } else {
                -1
            };
            let time = if chroma_vectors.len() <= 1 {
                0.0
            } else {
                index as f64 * duration / (chroma_vectors.len() - 1) as f64
            };
            json!({
                "time": round6(time),
                "chroma": normalized.into_iter().map(round6).collect::<Vec<_>>(),
                "color": if dominant < 0 { "#00000000".to_string() } else { color_for_pitch_class(dominant as usize) },
                "dominant_pitch_class": dominant,
            })
        })
        .collect()
}

fn harmonic_change_markers(frames: &[Value], max_markers: usize) -> Vec<AnalysisMarker> {
    let mut markers = Vec::new();
    let mut previous = None;
    for frame in frames {
        let Some(current) = frame.get("dominant_pitch_class").and_then(Value::as_i64) else {
            continue;
        };
        if current < 0 {
            continue;
        }
        if let Some(previous_pitch_class) = previous {
            if current != previous_pitch_class {
                let mut metadata = JsonObject::new();
                metadata.insert(
                    "previous_pitch_class".to_string(),
                    json!(previous_pitch_class),
                );
                metadata.insert("pitch_class".to_string(), json!(current));
                markers.push(AnalysisMarker {
                    timestamp: round6(
                        frame
                            .get("time")
                            .and_then(Value::as_f64)
                            .unwrap_or_default(),
                    ),
                    label: "Harmonic Change".to_string(),
                    category: "harmonic_change".to_string(),
                    confidence: Some(0.75),
                    metadata,
                });
            }
        }
        previous = Some(current);
        if markers.len() >= max_markers {
            break;
        }
    }
    markers
}

fn normalize(values: &[f64]) -> Vec<f64> {
    if values.is_empty() {
        return Vec::new();
    }
    let finite_values = values
        .iter()
        .copied()
        .map(|value| if value.is_finite() { value } else { 0.0 })
        .collect::<Vec<_>>();
    let min_value = finite_values.iter().copied().fold(f64::INFINITY, f64::min);
    let max_value = finite_values
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    if !min_value.is_finite() || !max_value.is_finite() || max_value <= min_value {
        return vec![0.0; finite_values.len()];
    }
    finite_values
        .into_iter()
        .map(|value| ((value - min_value) / (max_value - min_value)).clamp(0.0, 1.0))
        .collect()
}

fn estimate_tempo(beat_times: &[f64]) -> f64 {
    let intervals = beat_times
        .windows(2)
        .filter_map(|window| {
            let interval = window[1] - window[0];
            (interval.is_finite() && interval > 0.0).then_some(interval)
        })
        .collect::<Vec<_>>();
    if intervals.is_empty() {
        return 0.0;
    }
    round6(60.0 / (intervals.iter().sum::<f64>() / intervals.len() as f64))
}

fn frame_time(frame: &Value) -> Option<f64> {
    match frame.get("time")? {
        Value::Number(number) => number.as_f64(),
        Value::String(value) => value.parse().ok(),
        _ => None,
    }
}

fn color_for_pitch_class(pitch_class: usize) -> String {
    format!("hsl({}, 72%, 58%)", (pitch_class % 12) * 30)
}

fn validate_positive(value: usize, name: &'static str) -> Result<(), MusicError> {
    if value == 0 {
        Err(MusicError::InvalidPositiveInteger(name))
    } else {
        Ok(())
    }
}

fn raise_if_cancelled(cancel_requested: &mut impl FnMut() -> bool) -> Result<(), MusicError> {
    if cancel_requested() {
        Err(MusicError::Cancelled)
    } else {
        Ok(())
    }
}

fn finite_non_negative(value: f64) -> f64 {
    if value.is_finite() && value >= 0.0 {
        value
    } else {
        0.0
    }
}

fn round6(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        analyze_energy_fixture, analyze_harmonic_fixture, analyze_rhythm_fixture,
        visible_analysis_frames, MusicError,
    };

    #[test]
    fn music_energy_profile_returns_versioned_payload_frames_and_peak_markers() {
        let result =
            analyze_energy_fixture(4.0, &[0.0, 0.2, 1.0, 0.1, 0.0], 32, 4, || false).unwrap();

        assert_eq!(result.kind, "energy");
        assert_eq!(result.payload["version"], 1);
        assert_eq!(result.payload["kind"], "energy");
        assert_eq!(result.payload["settings"]["max_markers"], 4);
        assert!(result.frames.len() <= 32);
        assert!(result
            .frames
            .iter()
            .all(|frame| frame["intensity"].as_f64().unwrap() >= 0.0
                && frame["intensity"].as_f64().unwrap() <= 1.0));
        assert!(result
            .markers
            .iter()
            .any(|marker| marker.category == "energy_peak"));
    }

    #[test]
    fn music_beat_grid_returns_artifact_payload_and_marker_categories() {
        let result = analyze_rhythm_fixture(4.0, &[0.0, 0.5, 1.0, 1.5], 64, || false).unwrap();

        assert_eq!(result.kind, "beat-grid");
        assert_eq!(result.payload["version"], 1);
        assert_eq!(result.payload["kind"], "beat-grid");
        assert!(result
            .markers
            .iter()
            .all(|marker| marker.category == "beat"));
        assert!(result
            .markers
            .iter()
            .all(|marker| !marker.metadata.contains_key("meter")));
    }

    #[test]
    fn music_beat_grid_tempo_uses_emitted_filtered_beats() {
        let result =
            analyze_rhythm_fixture(4.0, &[-1.0, 0.0, 0.5, f64::NAN, 9.0], 2, || false).unwrap();

        assert_eq!(result.payload["beat_times"], json!([0.0, 0.5]));
        assert_eq!(result.payload["tempo"], json!(120.0));
        assert_eq!(result.markers[0].metadata["tempo"], json!(120.0));
    }

    #[test]
    fn music_energy_peak_detection_excludes_boundary_only_peaks() {
        let first_only = analyze_energy_fixture(2.0, &[1.0, 0.0, 0.0], 32, 8, || false).unwrap();
        let last_only = analyze_energy_fixture(2.0, &[0.0, 0.0, 1.0], 32, 8, || false).unwrap();
        let interior = analyze_energy_fixture(2.0, &[0.0, 1.0, 0.0], 32, 8, || false).unwrap();

        assert!(first_only.markers.is_empty());
        assert!(last_only.markers.is_empty());
        assert_eq!(interior.markers.len(), 1);
        assert_eq!(interior.markers[0].category, "energy_peak");
    }

    #[test]
    fn music_harmonic_profile_returns_color_frames_and_change_markers() {
        let mut c = [[0.0; 12]; 4];
        c[0][0] = 1.0;
        c[2][0] = 1.0;
        c[3][7] = 1.0;

        let result = analyze_harmonic_fixture(3.0, &c, 16, 16, || false).unwrap();

        assert_eq!(result.kind, "harmonic-color");
        assert_eq!(result.payload["version"], 1);
        assert_eq!(result.frames[1]["dominant_pitch_class"], -1);
        assert_eq!(result.markers[0].category, "harmonic_change");
        assert_eq!(result.markers[0].timestamp, 3.0);
    }

    #[test]
    fn music_visible_frames_return_bounded_window_and_preserve_kind() {
        let payload = json!({
            "version": 1,
            "kind": "energy",
            "duration": 10.0,
            "frames": (0..10).map(|index| json!({"time": index as f64, "intensity": index as f64 / 10.0})).collect::<Vec<_>>(),
        });

        let visible = visible_analysis_frames(&payload, 2.0, 3.0, 4).unwrap();

        assert_eq!(
            visible
                .frames
                .iter()
                .map(|frame| frame["time"].as_f64().unwrap())
                .collect::<Vec<_>>(),
            [2.0, 3.0, 4.0, 5.0]
        );
        assert_eq!(visible.kind, "energy");
    }

    #[test]
    fn music_visible_frames_exclude_malformed_times_and_preserve_left_edge_harmonic_context() {
        let energy_payload = json!({
            "kind": "energy",
            "frames": [
                {"id": "missing"},
                {"id": "bad", "time": "not-a-time"},
                {"id": "valid-zero", "time": 0.0},
                {"id": "valid-coerced", "time": "1.0"}
            ],
        });
        let harmonic_payload = json!({
            "kind": "harmonic-color",
            "frames": [
                {"time": 2.0, "color": "#f00"},
                {"time": 3.0, "color": "#0f0"},
                {"time": 4.0, "color": "#00f"}
            ],
        });

        let energy = visible_analysis_frames(&energy_payload, 0.0, 2.0, 512).unwrap();
        let harmonic = visible_analysis_frames(&harmonic_payload, 2.3, 1.5, 512).unwrap();

        assert_eq!(energy.frames[0]["id"], "valid-zero");
        assert_eq!(energy.frames[1]["id"], "valid-coerced");
        assert_eq!(harmonic.frames[0]["time"], 2.3);
        assert_eq!(harmonic.frames[0]["color"], "#f00");
        assert_eq!(harmonic.frames[1]["time"], 3.0);
    }

    #[test]
    fn music_helpers_validate_numeric_bounds_before_work() {
        let error = analyze_energy_fixture(1.0, &[0.0], 0, 1, || false).unwrap_err();

        assert!(matches!(
            error,
            MusicError::InvalidPositiveInteger("max_frames")
        ));
    }

    #[test]
    fn music_helpers_observe_cancellation_between_stages() {
        let mut checks = 0;
        let error = analyze_energy_fixture(1.0, &[0.0, 1.0], 32, 4, || {
            checks += 1;
            checks > 1
        })
        .unwrap_err();

        assert!(matches!(error, MusicError::Cancelled));
        assert!(checks > 1);
    }
}
