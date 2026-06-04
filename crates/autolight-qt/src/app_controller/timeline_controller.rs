use std::collections::BTreeSet;

use autolight_core::project::{ResultState, TrackType};
use cxx_qt_lib::QString;
use serde_json::{json, Value};

use crate::timeline_model::{
    timeline_rows_json_with_state, timeline_track_ids_for_project_with_state,
};
use crate::transform_model::transform_specs_json;

use super::{
    finite_duration, finite_non_negative, is_timing_snap_category, marker_end_seconds,
    AppControllerState, SNAP_THRESHOLD_PIXELS, TIMELINE_DEFAULT_PIXELS_PER_SECOND,
    TIMELINE_DEFAULT_VISIBLE_SECONDS, TIMELINE_MAX_PIXELS_PER_SECOND,
    TIMELINE_MIN_PIXELS_PER_SECOND, TIMELINE_MIN_VISIBLE_SECONDS,
};

#[derive(Clone, Debug)]
pub(super) struct TimelineControllerState {
    duration_seconds: f64,
    pixels_per_second: f64,
    scroll_seconds: f64,
    visible_seconds: f64,
    visible_track_range: Option<(usize, usize)>,
    visible_track_ids: BTreeSet<String>,
}

impl Default for TimelineControllerState {
    fn default() -> Self {
        Self {
            duration_seconds: 0.0,
            pixels_per_second: TIMELINE_DEFAULT_PIXELS_PER_SECOND,
            scroll_seconds: 0.0,
            visible_seconds: TIMELINE_DEFAULT_VISIBLE_SECONDS,
            visible_track_range: None,
            visible_track_ids: BTreeSet::new(),
        }
    }
}

impl TimelineControllerState {
    pub(super) fn duration_seconds(&self) -> f64 {
        self.duration_seconds
    }

    pub(super) fn pixels_per_second(&self) -> f64 {
        self.pixels_per_second
    }

    pub(super) fn scroll_seconds(&self) -> f64 {
        self.scroll_seconds
    }

    pub(super) fn visible_seconds(&self) -> f64 {
        self.visible_seconds
    }

    fn visible_track_range(&self) -> Option<(usize, usize)> {
        self.visible_track_range
    }

    fn has_visible_track_range(&self) -> bool {
        self.visible_track_range.is_some()
    }

    fn visible_track_ids(&self) -> BTreeSet<String> {
        self.visible_track_ids.clone()
    }

    fn set_duration(&mut self, duration_seconds: f64) {
        self.duration_seconds = finite_non_negative(duration_seconds);
        self.clamp_scroll();
    }

    fn set_zoom(&mut self, pixels_per_second: f64) {
        if !pixels_per_second.is_finite() {
            return;
        }
        self.pixels_per_second = pixels_per_second.clamp(
            TIMELINE_MIN_PIXELS_PER_SECOND,
            TIMELINE_MAX_PIXELS_PER_SECOND,
        );
        self.clamp_scroll();
    }

    fn set_scroll_seconds(&mut self, seconds: f64) {
        if !seconds.is_finite() {
            return;
        }
        self.scroll_seconds = finite_non_negative(seconds);
        self.clamp_scroll();
    }

    fn set_visible_seconds(&mut self, seconds: f64) {
        if !seconds.is_finite() {
            return;
        }
        self.visible_seconds = finite_non_negative(seconds).max(TIMELINE_MIN_VISIBLE_SECONDS);
        self.clamp_scroll();
    }

    fn set_visible_track_range(&mut self, first_row: usize, row_count: usize) {
        self.visible_track_range = Some((first_row, row_count));
    }

    fn set_visible_track_ids(&mut self, visible_track_ids: BTreeSet<String>) {
        self.visible_track_ids = visible_track_ids;
    }

    fn reset_view(&mut self) {
        self.pixels_per_second = TIMELINE_DEFAULT_PIXELS_PER_SECOND;
        self.scroll_seconds = 0.0;
        self.visible_seconds = TIMELINE_DEFAULT_VISIBLE_SECONDS;
        self.clamp_scroll();
    }

    fn restore_view(&mut self, ui_state: &Value) {
        self.pixels_per_second = ui_state
            .get("timeline")
            .and_then(|timeline| timeline.get("pixels_per_second"))
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
            .unwrap_or(TIMELINE_DEFAULT_PIXELS_PER_SECOND)
            .clamp(
                TIMELINE_MIN_PIXELS_PER_SECOND,
                TIMELINE_MAX_PIXELS_PER_SECOND,
            );
        self.scroll_seconds = ui_state
            .get("timeline")
            .and_then(|timeline| timeline.get("scroll_seconds"))
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
            .map(finite_non_negative)
            .unwrap_or(0.0);
        self.visible_seconds = TIMELINE_DEFAULT_VISIBLE_SECONDS;
    }

    fn capture_ui_state(&self, selected_track_id: &str) -> Value {
        json!({
            "selected_track_id": selected_track_id,
            "pixels_per_second": self.pixels_per_second,
            "scroll_seconds": self.scroll_seconds,
        })
    }

    fn clamp_scroll(&mut self) {
        let max_scroll = (self.duration_seconds - self.visible_seconds).max(0.0);
        self.scroll_seconds = self.scroll_seconds.clamp(0.0, max_scroll);
    }

    fn keep_time_visible(&mut self, seconds: f64) {
        let visible_seconds = self.visible_seconds.max(TIMELINE_MIN_VISIBLE_SECONDS);
        if seconds < self.scroll_seconds {
            self.scroll_seconds = seconds;
        } else if seconds > self.scroll_seconds + visible_seconds {
            self.scroll_seconds = seconds - visible_seconds;
        }
        self.clamp_scroll();
    }
}

impl AppControllerState {
    pub(super) fn sync_timeline_bridge_state(&mut self) {
        self.timeline_duration_seconds = self.timeline.duration_seconds();
        self.timeline_pixels_per_second = self.timeline.pixels_per_second();
        self.timeline_scroll_seconds = self.timeline.scroll_seconds();
        self.timeline_visible_seconds = self.timeline.visible_seconds();
        self.visible_track_range = self.timeline.visible_track_range();
        self.visible_track_ids = self.timeline.visible_track_ids();
    }

    pub(super) fn set_timeline_zoom_state(&mut self, pixels_per_second: f64) {
        self.timeline.set_zoom(pixels_per_second);
        self.sync_timeline_bridge_state();
        self.refresh_selected_state();
    }

    pub(super) fn set_timeline_scroll_seconds_state(&mut self, seconds: f64) {
        self.timeline.set_scroll_seconds(seconds);
        self.sync_timeline_bridge_state();
        self.refresh_selected_state();
    }

    pub(super) fn set_timeline_visible_seconds_state(&mut self, seconds: f64) {
        self.timeline.set_visible_seconds(seconds);
        self.sync_timeline_bridge_state();
        self.refresh_selected_state();
    }

    pub(super) fn set_timeline_visible_track_range_state(
        &mut self,
        first_row: i32,
        row_count: i32,
    ) {
        let first_row = first_row.max(0) as usize;
        let row_count = row_count.max(0) as usize;
        self.timeline.set_visible_track_range(first_row, row_count);
        self.refresh_visible_track_ids();
    }

    pub(super) fn snap_timeline_time_state(&self, seconds: f64, bypass_snap: bool) -> f64 {
        self.snap_timeline_time_excluding(seconds, bypass_snap, &BTreeSet::new())
    }

    pub(super) fn snap_timeline_time_excluding(
        &self,
        seconds: f64,
        bypass_snap: bool,
        excluded_marker_ids: &BTreeSet<String>,
    ) -> f64 {
        if bypass_snap || !seconds.is_finite() {
            return seconds;
        }
        let threshold_seconds = SNAP_THRESHOLD_PIXELS / self.timeline.pixels_per_second().max(1.0);
        let visible_track_ids = self.visible_track_ids();
        let eligible_track_ids = self.eligible_snap_track_ids(&visible_track_ids);
        self.project
            .markers
            .iter()
            .filter(|marker| eligible_track_ids.contains(&marker.track_id))
            .filter(|marker| !excluded_marker_ids.contains(&marker.id))
            .filter(|marker| is_timing_snap_category(&marker.category))
            .filter_map(|marker| {
                let distance = (marker.timestamp - seconds).abs();
                (distance <= threshold_seconds).then_some((distance, marker.timestamp))
            })
            .min_by(|left, right| left.0.total_cmp(&right.0))
            .map(|(_, timestamp)| timestamp)
            .map_or_else(|| finite_non_negative(seconds), finite_non_negative)
    }

    pub(super) fn refresh_view_state(&mut self) {
        let selected_marker_ids = self.selected_marker_ids_set();
        match timeline_rows_json_with_state(
            &self.project,
            &self.expanded_track_ids,
            &selected_marker_ids,
        ) {
            Ok(rows_json) => {
                self.timeline_rows_json = QString::from(&rows_json);
            }
            Err(error) => {
                self.set_error(error.to_string());
            }
        }
        self.transform_specs_json = QString::from(
            &transform_specs_json(&self.transform_registry).unwrap_or_else(|_| "[]".to_string()),
        );
        let duration_seconds = self.project_timeline_duration_seconds();
        self.timeline.set_duration(duration_seconds);
        self.refresh_visible_track_ids();
        self.refresh_selected_state();
    }

    pub(super) fn capture_timeline_ui_state(&mut self) {
        self.project.ui_state.insert(
            "expanded_track_ids".to_string(),
            json!(self.expanded_track_ids.iter().cloned().collect::<Vec<_>>()),
        );
        self.project.ui_state.insert(
            "timeline".to_string(),
            self.timeline
                .capture_ui_state(&self.selected_track_id.to_string()),
        );
    }

    pub(super) fn restore_timeline_view_state(&mut self) {
        self.timeline
            .restore_view(&Value::Object(self.project.ui_state.clone()));
        self.sync_timeline_bridge_state();
    }

    pub(super) fn reset_timeline_view_state(&mut self) {
        self.timeline.reset_view();
        self.sync_timeline_bridge_state();
    }

    pub(super) fn keep_timeline_time_visible(&mut self, seconds: f64) {
        self.timeline.keep_time_visible(seconds);
        self.sync_timeline_bridge_state();
    }

    pub(super) fn project_timeline_duration_seconds(&self) -> f64 {
        let audio_duration = self
            .project
            .audio_assets
            .iter()
            .filter_map(|asset| finite_duration(asset.duration))
            .fold(self.playback.duration_seconds(), f64::max);
        self.project
            .markers
            .iter()
            .filter_map(marker_end_seconds)
            .fold(audio_duration, f64::max)
    }

    pub(super) fn visible_track_ids(&self) -> BTreeSet<String> {
        if self.timeline.has_visible_track_range() {
            return self.timeline.visible_track_ids();
        }
        self.timeline_track_ids()
    }

    pub(super) fn timeline_track_ids(&self) -> BTreeSet<String> {
        timeline_track_ids_for_project_with_state(&self.project, &self.expanded_track_ids)
            .into_iter()
            .collect()
    }

    pub(super) fn eligible_snap_track_ids(
        &self,
        visible_track_ids: &BTreeSet<String>,
    ) -> BTreeSet<String> {
        self.project
            .tracks
            .iter()
            .filter(|track| visible_track_ids.contains(&track.id))
            .filter(|track| track.track_type == TrackType::Generated)
            .filter(|track| track.result_state == ResultState::Complete)
            .map(|track| track.id.clone())
            .collect()
    }

    pub(super) fn refresh_visible_track_ids(&mut self) {
        let visible_track_ids =
            if let Some((first_row, row_count)) = self.timeline.visible_track_range() {
                timeline_track_ids_for_project_with_state(&self.project, &self.expanded_track_ids)
                    .into_iter()
                    .skip(first_row)
                    .take(row_count)
                    .collect()
            } else {
                self.timeline_track_ids()
            };
        self.timeline.set_visible_track_ids(visible_track_ids);
        self.sync_timeline_bridge_state();
    }
}
