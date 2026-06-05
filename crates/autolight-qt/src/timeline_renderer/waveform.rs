use autolight_analysis::waveform::{WaveformLevel, WaveformPayload, WaveformSample};
use serde_json::Value;

const MAX_RENDER_COLUMNS: usize = 2_048;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaveformPeakColumn {
    pub min: f32,
    pub max: f32,
    pub rms: f32,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WaveformPeakLevel {
    pub samples_per_column: u32,
    pub columns: Vec<WaveformPeakColumn>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WaveformPeakPyramid {
    pub sample_rate: u32,
    pub frame_count: u64,
    pub levels: Vec<WaveformPeakLevel>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaveformRenderRequest {
    pub scroll_seconds: f64,
    pub visible_seconds: f64,
    pub pixels_per_second: f64,
    pub width_pixels: f64,
    pub height_pixels: f64,
    pub left_padding_pixels: f64,
    pub device_pixel_ratio: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WaveformRenderFrame {
    PeakColumns(Vec<WaveformColumnGeometry>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaveformColumnGeometry {
    pub x: f32,
    pub min_y: f32,
    pub max_y: f32,
    pub rms_top_y: f32,
    pub rms_bottom_y: f32,
    pub width: f32,
}

impl WaveformPeakPyramid {
    pub fn from_payload(payload: &WaveformPayload) -> Self {
        let duration = payload.duration.max(0.0);
        let frame_count = frame_count(payload.sample_rate, duration);
        let mut levels = if payload.levels.is_empty() {
            let samples_per_column = samples_per_column(frame_count, payload.samples.len());
            vec![WaveformPeakLevel {
                samples_per_column,
                columns: payload_samples_to_columns(&payload.samples, samples_per_column),
            }]
        } else {
            payload
                .levels
                .iter()
                .filter_map(|level| level_to_peak_level(level, frame_count))
                .collect()
        };
        levels.sort_by_key(|level| level.samples_per_column);
        Self {
            sample_rate: payload.sample_rate,
            frame_count,
            levels,
        }
    }

    pub fn from_json(value: &Value) -> Result<Self, serde_json::Error> {
        let payload: WaveformPayload = if value.get("levels").and_then(Value::as_array).is_some() {
            serde_json::from_value(value.clone())?
        } else {
            WaveformPayload {
                version: value
                    .get("version")
                    .and_then(Value::as_u64)
                    .and_then(|version| u32::try_from(version).ok())
                    .unwrap_or(1),
                sample_rate: value
                    .get("sample_rate")
                    .or_else(|| value.get("sampleRate"))
                    .and_then(Value::as_u64)
                    .and_then(|sample_rate| u32::try_from(sample_rate).ok())
                    .unwrap_or(0),
                duration: value.get("duration").and_then(Value::as_f64).unwrap_or(0.0),
                samples: serde_json::from_value(
                    value
                        .get("samples")
                        .cloned()
                        .unwrap_or_else(|| Value::Array(Vec::new())),
                )?,
                levels: Vec::new(),
            }
        };
        Ok(Self::from_payload(&payload))
    }

    pub fn render(&self, request: WaveformRenderRequest) -> WaveformRenderFrame {
        if self.sample_rate == 0
            || self.frame_count == 0
            || !request.width_pixels.is_finite()
            || request.width_pixels <= 0.0
            || !request.height_pixels.is_finite()
            || request.height_pixels <= 0.0
            || !request.pixels_per_second.is_finite()
            || request.pixels_per_second <= 0.0
            || !request.scroll_seconds.is_finite()
            || !request.visible_seconds.is_finite()
            || !request.left_padding_pixels.is_finite()
            || !request.device_pixel_ratio.is_finite()
        {
            return WaveformRenderFrame::PeakColumns(Vec::new());
        }

        let samples_per_pixel = f64::from(self.sample_rate) / request.pixels_per_second;
        let Some(level) = self.select_level(samples_per_pixel) else {
            return WaveformRenderFrame::PeakColumns(Vec::new());
        };
        WaveformRenderFrame::PeakColumns(project_peak_columns(level, self, request))
    }

    fn select_level(&self, samples_per_pixel: f64) -> Option<&WaveformPeakLevel> {
        self.levels
            .iter()
            .filter(|level| f64::from(level.samples_per_column) <= samples_per_pixel)
            .max_by_key(|level| level.samples_per_column)
            .or_else(|| self.levels.first())
    }

    fn duration_seconds(&self) -> f64 {
        self.frame_count as f64 / f64::from(self.sample_rate)
    }
}

fn project_peak_columns(
    level: &WaveformPeakLevel,
    pyramid: &WaveformPeakPyramid,
    request: WaveformRenderRequest,
) -> Vec<WaveformColumnGeometry> {
    let duration = pyramid.duration_seconds();
    if duration <= 0.0 || level.columns.is_empty() {
        return Vec::new();
    }
    let output_limit = visible_pixel_limit(request);
    let first_time = request.scroll_seconds.max(0.0);
    let last_time = (first_time + request.visible_seconds.max(0.0)).min(duration);
    let first_pixel = request.left_padding_pixels.max(0.0).floor();
    let last_pixel = request.width_pixels.max(request.left_padding_pixels).ceil();
    let pixel_span = last_pixel - first_pixel;
    if pixel_span <= 0.0 {
        return Vec::new();
    }
    let column_count = output_limit.min(pixel_span.ceil().max(1.0) as usize);
    let mut columns = Vec::new();

    for column_index in 0..column_count {
        let start_x = first_pixel + pixel_span * column_index as f64 / column_count as f64;
        let stop_x = first_pixel + pixel_span * (column_index + 1) as f64 / column_count as f64;
        let pixel_time_start =
            first_time + (start_x - request.left_padding_pixels) / request.pixels_per_second;
        let pixel_time_stop =
            first_time + (stop_x - request.left_padding_pixels) / request.pixels_per_second;
        if pixel_time_stop < first_time || pixel_time_start > last_time {
            continue;
        }
        let start_index =
            ((pixel_time_start.max(0.0) / duration) * level.columns.len() as f64).floor() as usize;
        let stop_index = (((pixel_time_stop.min(duration) / duration) * level.columns.len() as f64)
            .ceil() as usize)
            .min(level.columns.len());
        if start_index >= stop_index {
            continue;
        }
        let aggregate = aggregate_columns(&level.columns[start_index..stop_index]);
        columns.push(column_geometry(
            start_x,
            stop_x - start_x,
            aggregate,
            request,
        ));
    }

    columns
}

fn column_geometry(
    x: f64,
    width: f64,
    column: WaveformPeakColumn,
    request: WaveformRenderRequest,
) -> WaveformColumnGeometry {
    let center_y = request.height_pixels / 2.0;
    let scale_y = (request.height_pixels - 4.0).max(1.0) / 2.0;
    let min_y = center_y - f64::from(column.max) * scale_y;
    let max_y = center_y - f64::from(column.min) * scale_y;
    let rms_height = f64::from(column.rms).abs() * scale_y;
    WaveformColumnGeometry {
        x: x as f32,
        min_y: min_y.min(max_y) as f32,
        max_y: max_y.max(min_y) as f32,
        rms_top_y: (center_y - rms_height) as f32,
        rms_bottom_y: (center_y + rms_height) as f32,
        width: width.max(1.0) as f32,
    }
}

fn aggregate_columns(columns: &[WaveformPeakColumn]) -> WaveformPeakColumn {
    let mut min = 0.0_f32;
    let mut max = 0.0_f32;
    let mut count = 0_u32;
    let mut sum_squares = 0.0_f64;
    for column in columns {
        min = min.min(column.min);
        max = max.max(column.max);
        count = count.saturating_add(column.count);
        sum_squares += f64::from(column.rms) * f64::from(column.rms) * f64::from(column.count);
    }
    let rms = if count == 0 {
        0.0
    } else {
        (sum_squares / f64::from(count)).sqrt() as f32
    };
    WaveformPeakColumn {
        min,
        max,
        rms,
        count,
    }
}

fn level_to_peak_level(level: &WaveformLevel, frame_count: u64) -> Option<WaveformPeakLevel> {
    if level.samples.is_empty() {
        return None;
    }
    let samples_per_column = samples_per_column(frame_count, level.samples.len());
    Some(WaveformPeakLevel {
        samples_per_column,
        columns: payload_samples_to_columns(&level.samples, samples_per_column),
    })
}

fn payload_samples_to_columns(
    samples: &[WaveformSample],
    samples_per_column: u32,
) -> Vec<WaveformPeakColumn> {
    samples
        .iter()
        .map(|sample| {
            let count = sample.count.try_into().unwrap_or(samples_per_column);
            let count = if count == 0 {
                samples_per_column
            } else {
                count
            };
            let peak = finite_unit(sample.peak.abs()) as f32;
            let rms = if sample.sum_squares.is_finite() && sample.sum_squares >= 0.0 && count > 0 {
                (sample.sum_squares / f64::from(count)).sqrt()
            } else {
                sample.rms
            };
            WaveformPeakColumn {
                min: -peak,
                max: peak,
                rms: finite_unit(rms) as f32,
                count,
            }
        })
        .collect()
}

fn frame_count(sample_rate: u32, duration: f64) -> u64 {
    if sample_rate == 0 || !duration.is_finite() || duration <= 0.0 {
        return 0;
    }
    (duration * f64::from(sample_rate)).round().max(0.0) as u64
}

fn samples_per_column(frame_count: u64, column_count: usize) -> u32 {
    if frame_count == 0 || column_count == 0 {
        return 1;
    }
    let columns = column_count as u64;
    frame_count.div_ceil(columns).try_into().unwrap_or(u32::MAX)
}

fn visible_pixel_limit(request: WaveformRenderRequest) -> usize {
    let width = (request.width_pixels * request.device_pixel_ratio.max(1.0))
        .ceil()
        .max(0.0) as usize;
    width.saturating_add(1).min(MAX_RENDER_COLUMNS)
}

fn finite_unit(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use autolight_analysis::waveform::{WaveformLevel, WaveformPayload, WaveformSample};
    use serde_json::json;

    use super::{WaveformPeakPyramid, WaveformRenderFrame, WaveformRenderRequest};

    #[test]
    fn waveform_projection_chooses_peak_columns_when_zoomed_out() {
        let pyramid = pyramid_with_levels(&[4, 16], 16.0);

        let frame = pyramid.render(request(0.0, 1.0, 8.0, 120.0));

        let WaveformRenderFrame::PeakColumns(columns) = frame;
        assert!(!columns.is_empty());
    }

    #[test]
    fn waveform_projection_keeps_peak_envelope_when_zoomed_in() {
        let pyramid = pyramid_with_levels(&[16], 16.0);

        let frame = pyramid.render(request(0.0, 1.0, 64.0, 120.0));

        let WaveformRenderFrame::PeakColumns(columns) = frame;
        assert!(!columns.is_empty());
        assert!(columns
            .iter()
            .any(|column| column.rms_bottom_y > column.rms_top_y));
    }

    #[test]
    fn waveform_projection_preserves_single_sample_impulse_when_zoomed_out() {
        let mut pyramid = pyramid_with_levels(&[100], 100.0);
        pyramid.levels[0].columns[50].max = 1.0;
        pyramid.levels[0].columns[50].min = -1.0;

        let frame = pyramid.render(request(0.0, 1.0, 10.0, 40.0));

        let WaveformRenderFrame::PeakColumns(columns) = frame;
        assert!(columns
            .iter()
            .any(|column| column.min_y <= 2.0 && column.max_y >= 38.0));
    }

    #[test]
    fn waveform_projection_combines_rms_from_energy_not_average() {
        let mut pyramid = pyramid_with_levels(&[2], 2.0);
        pyramid.levels[0].columns[0].rms = 0.0;
        pyramid.levels[0].columns[0].count = 1;
        pyramid.levels[0].columns[1].rms = 1.0;
        pyramid.levels[0].columns[1].count = 9;

        let frame = pyramid.render(request(0.0, 1.0, 1.0, 40.0));

        let WaveformRenderFrame::PeakColumns(columns) = frame;
        let center = 20.0_f32;
        let rms_height = center - columns[0].rms_top_y;
        assert!((rms_height / 18.0 - (0.9_f32).sqrt()).abs() < 0.02);
    }

    #[test]
    fn waveform_projection_output_is_bounded_by_visible_width() {
        let pyramid = pyramid_with_levels(&[1_000], 1_000.0);

        let frame = pyramid.render(request(0.0, 1.0, 1_000.0, 32.0));

        let WaveformRenderFrame::PeakColumns(columns) = frame;
        assert!(columns.len() <= 33);
    }

    #[test]
    fn waveform_projection_caps_wide_tiles_and_spans_full_width() {
        let pyramid = pyramid_with_levels(&[10_000], 10_000.0);

        let frame = pyramid.render(request(0.0, 1.0, 10_000.0, 10_000.0));

        let WaveformRenderFrame::PeakColumns(columns) = frame;
        assert!(columns.len() <= 2_049);
        let last = columns.last().unwrap();
        assert!(last.x + last.width >= 9_999.0);
        assert!(columns.iter().any(|column| column.width > 1.0));
    }

    #[test]
    fn waveform_payload_v1_converts_to_peak_pyramid_with_inferred_counts() {
        let value = json!({
            "version": 1,
            "sample_rate": 4,
            "duration": 1.0,
            "samples": [{"peak": 0.5, "rms": 0.25}]
        });

        let pyramid = WaveformPeakPyramid::from_json(&value).unwrap();

        assert_eq!(pyramid.levels[0].columns[0].min, -0.5);
        assert_eq!(pyramid.levels[0].columns[0].max, 0.5);
        assert_eq!(pyramid.levels[0].columns[0].count, 4);
    }

    #[test]
    fn waveform_payload_v2_converts_to_peak_pyramid_preserving_sum_squares() {
        let payload = WaveformPayload {
            version: 2,
            sample_rate: 10,
            duration: 1.0,
            samples: Vec::new(),
            levels: vec![WaveformLevel {
                bucket_count: 1,
                samples: vec![WaveformSample {
                    peak: 0.8,
                    rms: 0.0,
                    count: 10,
                    sum_squares: 2.5,
                }],
            }],
        };

        let pyramid = WaveformPeakPyramid::from_payload(&payload);

        assert!((pyramid.levels[0].columns[0].rms - 0.5).abs() < 1e-6);
        assert_eq!(pyramid.levels[0].columns[0].count, 10);
    }

    fn request(
        scroll_seconds: f64,
        visible_seconds: f64,
        pixels_per_second: f64,
        width_pixels: f64,
    ) -> WaveformRenderRequest {
        WaveformRenderRequest {
            scroll_seconds,
            visible_seconds,
            pixels_per_second,
            width_pixels,
            height_pixels: 40.0,
            left_padding_pixels: 0.0,
            device_pixel_ratio: 1.0,
        }
    }

    fn pyramid_with_levels(bucket_counts: &[usize], sample_rate: f64) -> WaveformPeakPyramid {
        let levels = bucket_counts
            .iter()
            .map(|count| WaveformLevel {
                bucket_count: *count,
                samples: (0..*count)
                    .map(|_| WaveformSample {
                        peak: 0.2,
                        rms: 0.1,
                        count: 1,
                        sum_squares: 0.01,
                    })
                    .collect(),
            })
            .collect();
        WaveformPeakPyramid::from_payload(&WaveformPayload {
            version: 2,
            sample_rate: sample_rate as u32,
            duration: 1.0,
            samples: Vec::new(),
            levels,
        })
    }
}
