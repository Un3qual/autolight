use serde::{Deserialize, Serialize};
use serde_json::Value;

use autolight_core::project::{ProjectDocument, Track};

use crate::timeline_model::{TimelineAnalysisRef, TimelineRow, TimelineWaveformRef};

pub const TIMELINE_LABEL_WIDTH: f64 = 280.0;
pub const TIMELINE_RULER_HEIGHT: f64 = 32.0;
pub const TIMELINE_ROW_HEIGHT: f64 = 76.0;
pub const TIMELINE_LEFT_PADDING: f64 = 24.0;
const MAX_SCENE_WAVEFORM_PREVIEW_SAMPLES: usize = 4_096;
const MAX_SCENE_ANALYSIS_PREVIEW_SAMPLES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSceneSnapshot {
    pub tracks: Vec<TimelineSceneTrack>,
    pub duration_seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSceneTrack {
    pub track_id: String,
    pub name: String,
    pub track_type: String,
    pub result_state: String,
    pub depth: usize,
    pub has_children: bool,
    pub selected: bool,
    pub expanded: bool,
    pub markers: Vec<TimelineSceneMarker>,
    pub waveform_ref: Option<TimelineSceneArtifactRef>,
    pub waveform_preview: Vec<TimelineSceneWaveformSample>,
    pub analysis_refs: Vec<TimelineSceneArtifactRef>,
    pub analysis_previews: Vec<TimelineSceneAnalysisPreview>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSceneMarker {
    pub marker_id: String,
    pub timestamp: f64,
    pub duration: f64,
    pub label: String,
    pub color: String,
    pub selected: bool,
    pub editable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSceneArtifactRef {
    pub track_id: String,
    pub cache_ref: String,
    pub artifact_kind: String,
    pub duration_seconds: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSceneWaveformSample {
    pub time: f64,
    pub peak: f64,
    pub rms: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSceneAnalysisPreview {
    pub artifact_kind: String,
    pub samples: Vec<TimelineSceneAnalysisSample>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSceneAnalysisSample {
    pub time: f64,
    pub intensity: f64,
    pub color: String,
}

pub fn scene_snapshot_from_rows(
    rows: &[TimelineRow],
    duration_seconds: f64,
) -> TimelineSceneSnapshot {
    scene_snapshot_from_rows_with_selection(rows, duration_seconds, "")
}

pub fn scene_snapshot_from_rows_with_selection(
    rows: &[TimelineRow],
    duration_seconds: f64,
    selected_track_id: &str,
) -> TimelineSceneSnapshot {
    TimelineSceneSnapshot {
        duration_seconds,
        tracks: rows
            .iter()
            .map(|row| TimelineSceneTrack::from_row(row, row.track_id == selected_track_id))
            .collect(),
    }
}

pub fn scene_snapshot_from_project_rows(
    project: &ProjectDocument,
    rows: &[TimelineRow],
    duration_seconds: f64,
    selected_track_id: &str,
) -> TimelineSceneSnapshot {
    TimelineSceneSnapshot {
        duration_seconds,
        tracks: rows
            .iter()
            .map(|row| {
                let track = project.tracks.iter().find(|track| track.id == row.track_id);
                let waveform_preview = track
                    .and_then(|track| track.provenance.get("waveform_payload"))
                    .map_or_else(Vec::new, |payload| {
                        waveform_preview_from_payload(payload, row.waveform_duration_seconds)
                    });
                let analysis_previews = track
                    .map(|track| analysis_previews_from_row(track, row))
                    .unwrap_or_default();
                TimelineSceneTrack::from_row_with_waveform_preview(
                    row,
                    row.track_id == selected_track_id,
                    waveform_preview,
                    analysis_previews,
                )
            })
            .collect(),
    }
}

impl From<&TimelineRow> for TimelineSceneTrack {
    fn from(row: &TimelineRow) -> Self {
        Self::from_row(row, false)
    }
}

impl TimelineSceneTrack {
    fn from_row(row: &TimelineRow, selected: bool) -> Self {
        Self::from_row_with_waveform_preview(row, selected, Vec::new(), Vec::new())
    }

    fn from_row_with_waveform_preview(
        row: &TimelineRow,
        selected: bool,
        waveform_preview: Vec<TimelineSceneWaveformSample>,
        analysis_previews: Vec<TimelineSceneAnalysisPreview>,
    ) -> Self {
        Self {
            track_id: row.track_id.clone(),
            name: row.name.clone(),
            track_type: row.track_type.clone(),
            result_state: row.result_state.clone(),
            depth: row.depth,
            has_children: row.has_children,
            selected,
            expanded: row.expanded,
            markers: row
                .marker_spans
                .iter()
                .map(|marker| TimelineSceneMarker {
                    marker_id: marker.id.clone(),
                    timestamp: marker.timestamp,
                    duration: marker.duration,
                    label: marker.label.clone(),
                    color: marker.color.clone(),
                    selected: marker.selected,
                    editable: row.editable,
                })
                .collect(),
            waveform_ref: row
                .waveform_ref
                .as_ref()
                .map(TimelineSceneArtifactRef::from),
            waveform_preview,
            analysis_refs: row
                .analysis_refs
                .iter()
                .map(TimelineSceneArtifactRef::from)
                .collect(),
            analysis_previews,
        }
    }
}

fn waveform_preview_from_payload(
    payload: &Value,
    fallback_duration_seconds: f64,
) -> Vec<TimelineSceneWaveformSample> {
    let duration_seconds = payload
        .get("duration")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(fallback_duration_seconds);
    if duration_seconds <= 0.0 {
        return Vec::new();
    }
    let samples = waveform_preview_source_samples(payload);
    if samples.is_empty() {
        return Vec::new();
    }
    let stride = samples
        .len()
        .div_ceil(MAX_SCENE_WAVEFORM_PREVIEW_SAMPLES)
        .max(1);
    let bucket_count = samples.len();
    samples
        .into_iter()
        .enumerate()
        .step_by(stride)
        .map(|(index, sample)| TimelineSceneWaveformSample {
            time: index as f64 * duration_seconds / bucket_count.max(1) as f64,
            peak: sample
                .get("peak")
                .and_then(Value::as_f64)
                .map(finite_unit)
                .unwrap_or(0.0),
            rms: sample
                .get("rms")
                .and_then(Value::as_f64)
                .map(finite_unit)
                .unwrap_or(0.0),
        })
        .collect()
}

fn waveform_preview_source_samples(payload: &Value) -> Vec<Value> {
    payload
        .get("levels")
        .and_then(Value::as_array)
        .and_then(|levels| {
            levels
                .iter()
                .filter_map(|level| {
                    let samples = level.get("samples").and_then(Value::as_array)?;
                    Some((samples.len(), samples))
                })
                .filter(|(_, samples)| !samples.is_empty())
                .min_by_key(|(len, _)| len.abs_diff(MAX_SCENE_WAVEFORM_PREVIEW_SAMPLES))
                .map(|(_, samples)| samples.clone())
        })
        .or_else(|| payload.get("samples").and_then(Value::as_array).cloned())
        .unwrap_or_default()
}

fn finite_unit(value: f64) -> f64 {
    if value.is_finite() {
        value.abs().clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn analysis_previews_from_row(
    track: &Track,
    row: &TimelineRow,
) -> Vec<TimelineSceneAnalysisPreview> {
    row.analysis_refs
        .iter()
        .filter_map(|reference| {
            let provenance_key = match reference.artifact_kind.as_str() {
                "energy" => "visible_energy",
                "harmonic-color" => "visible_harmonic_color",
                _ => return None,
            };
            let payload = track.provenance.get(provenance_key)?;
            let samples = analysis_preview_samples_from_payload(payload);
            if samples.is_empty() {
                None
            } else {
                Some(TimelineSceneAnalysisPreview {
                    artifact_kind: reference.artifact_kind.clone(),
                    samples,
                })
            }
        })
        .collect()
}

fn analysis_preview_samples_from_payload(payload: &Value) -> Vec<TimelineSceneAnalysisSample> {
    let samples = payload
        .as_array()
        .or_else(|| {
            payload
                .get("samples")
                .or_else(|| payload.get("frames"))
                .and_then(Value::as_array)
        })
        .cloned()
        .unwrap_or_default();
    if samples.is_empty() {
        return analysis_preview_bin_samples(payload);
    }
    let stride = samples
        .len()
        .div_ceil(MAX_SCENE_ANALYSIS_PREVIEW_SAMPLES)
        .max(1);
    samples
        .into_iter()
        .enumerate()
        .step_by(stride)
        .map(|(index, sample)| TimelineSceneAnalysisSample {
            time: sample
                .get("time")
                .or_else(|| sample.get("timestamp"))
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite() && *value >= 0.0)
                .unwrap_or(index as f64),
            intensity: sample
                .get("intensity")
                .or_else(|| sample.get("value"))
                .or_else(|| sample.get("energy"))
                .and_then(Value::as_f64)
                .map(finite_unit)
                .unwrap_or(1.0),
            color: sample
                .get("color")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("#93c5fd")
                .to_string(),
        })
        .collect()
}

fn analysis_preview_bin_samples(payload: &Value) -> Vec<TimelineSceneAnalysisSample> {
    let Some(bins) = payload.get("bins").and_then(Value::as_array) else {
        return Vec::new();
    };
    let sample_rate = payload
        .get("sample_rate")
        .or_else(|| payload.get("sampleRate"))
        .and_then(Value::as_f64)
        .filter(|rate| rate.is_finite() && *rate > 0.0);
    let stride = bins
        .len()
        .div_ceil(MAX_SCENE_ANALYSIS_PREVIEW_SAMPLES)
        .max(1);
    bins.iter()
        .enumerate()
        .step_by(stride)
        .filter_map(|(index, bin)| {
            Some(TimelineSceneAnalysisSample {
                time: sample_rate.map_or(index as f64, |rate| index as f64 / rate),
                intensity: finite_unit(bin.as_f64()?),
                color: "#93c5fd".to_string(),
            })
        })
        .collect()
}

impl From<&TimelineWaveformRef> for TimelineSceneArtifactRef {
    fn from(value: &TimelineWaveformRef) -> Self {
        Self {
            track_id: value.track_id.clone(),
            cache_ref: value.cache_ref.clone(),
            artifact_kind: value.artifact_kind.clone(),
            duration_seconds: value.duration_seconds,
        }
    }
}

impl From<&TimelineAnalysisRef> for TimelineSceneArtifactRef {
    fn from(value: &TimelineAnalysisRef) -> Self {
        Self {
            track_id: value.track_id.clone(),
            cache_ref: value.cache_ref.clone(),
            artifact_kind: value.artifact_kind.clone(),
            duration_seconds: value.duration_seconds,
        }
    }
}
