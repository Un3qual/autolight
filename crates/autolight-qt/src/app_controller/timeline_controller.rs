use std::collections::BTreeSet;

use autolight_core::project::{ResultState, TrackType};
use cxx_qt_lib::QString;
use serde_json::{json, Value};

use crate::timeline_model::{
    timeline_rows_for_project_with_state, timeline_track_ids_for_project_with_state,
};
use crate::timeline_scene::scene_snapshot_from_project_rows;
use crate::transform_model::transform_specs_json;

use super::{
    finite_duration, finite_non_negative, is_timing_snap_category, marker_end_seconds,
    timeline_viewport::{
        apply_follow, playhead_offscreen_direction_for, scroll_by_pixels, scrub_at_x,
        visible_seconds_for_lane, zoom_anchor_for_pointer_or_playhead, zoom_by_factor,
        PixelsPerSecond, PlayheadOffscreenDirection, TimelineFollowMode, TimelinePixels,
        TimelineSeconds, TimelineViewport, TimelineZoomBounds, DEFAULT_PIXELS_PER_SECOND,
        DEFAULT_VISIBLE_SECONDS, MAX_PIXELS_PER_SECOND, PLAYHEAD_ANCHOR_THRESHOLD_PIXELS,
    },
    AppControllerState, SNAP_THRESHOLD_PIXELS,
};

#[derive(Clone, Debug)]
pub(super) struct TimelineControllerState {
    duration_seconds: f64,
    pixels_per_second: f64,
    scroll_seconds: f64,
    visible_seconds: f64,
    lane_width_pixels: f64,
    visible_track_range: Option<(usize, usize)>,
    visible_track_ids: BTreeSet<String>,
    follow_mode: TimelineFollowMode,
    user_navigation_active: bool,
    playhead_offscreen_direction: PlayheadOffscreenDirection,
}

impl Default for TimelineControllerState {
    fn default() -> Self {
        Self {
            duration_seconds: 0.0,
            pixels_per_second: DEFAULT_PIXELS_PER_SECOND,
            scroll_seconds: 0.0,
            visible_seconds: DEFAULT_VISIBLE_SECONDS,
            lane_width_pixels: 0.0,
            visible_track_range: None,
            visible_track_ids: BTreeSet::new(),
            follow_mode: TimelineFollowMode::Center,
            user_navigation_active: false,
            playhead_offscreen_direction: PlayheadOffscreenDirection::Visible,
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

    pub(super) fn min_pixels_per_second(&self) -> f64 {
        TimelineZoomBounds::for_lane(
            TimelineSeconds::new(self.duration_seconds),
            TimelinePixels::non_negative(self.lane_width_pixels),
        )
        .minimum()
        .value()
    }

    pub(super) fn max_pixels_per_second(&self) -> f64 {
        MAX_PIXELS_PER_SECOND
    }

    pub(super) fn follow_mode(&self) -> TimelineFollowMode {
        self.follow_mode
    }

    pub(super) fn user_navigation_active(&self) -> bool {
        self.user_navigation_active
    }

    pub(super) fn playhead_offscreen_direction(&self) -> PlayheadOffscreenDirection {
        self.playhead_offscreen_direction
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

    fn viewport(&self) -> TimelineViewport {
        TimelineViewport::new(
            self.duration_seconds,
            self.pixels_per_second,
            self.scroll_seconds,
            self.visible_seconds,
        )
    }

    fn apply_viewport(&mut self, viewport: TimelineViewport) {
        self.duration_seconds = viewport.duration().value();
        self.pixels_per_second = viewport.pixels_per_second().value();
        self.scroll_seconds = viewport.scroll().value();
        self.visible_seconds = viewport.visible().value();
    }

    fn refresh_playhead_offscreen_direction(&mut self, position_seconds: f64) {
        self.playhead_offscreen_direction = playhead_offscreen_direction_for(
            self.viewport(),
            TimelineSeconds::new(position_seconds),
        );
    }

    fn set_duration(&mut self, duration_seconds: f64) {
        let viewport = TimelineViewport::new(
            duration_seconds,
            self.pixels_per_second,
            self.scroll_seconds,
            self.visible_seconds,
        );
        self.apply_viewport(viewport);
    }

    fn set_zoom(&mut self, pixels_per_second: f64) {
        let bounds =
            TimelineZoomBounds::default_for_duration(TimelineSeconds::new(self.duration_seconds));
        let viewport = self
            .viewport()
            .with_pixels_per_second(PixelsPerSecond::clamped(pixels_per_second, bounds), None);
        self.apply_viewport(viewport);
    }

    fn set_scroll_seconds(&mut self, seconds: f64) {
        let viewport = self.viewport().with_scroll(TimelineSeconds::new(seconds));
        self.apply_viewport(viewport);
    }

    fn set_visible_seconds(&mut self, seconds: f64) {
        let viewport = self.viewport().with_visible(TimelineSeconds::new(seconds));
        self.apply_viewport(viewport);
    }

    fn set_visible_track_range(&mut self, first_row: usize, row_count: usize) {
        self.visible_track_range = Some((first_row, row_count));
    }

    fn set_visible_track_ids(&mut self, visible_track_ids: BTreeSet<String>) {
        self.visible_track_ids = visible_track_ids;
    }

    fn reset_view(&mut self) {
        self.pixels_per_second = DEFAULT_PIXELS_PER_SECOND;
        self.scroll_seconds = 0.0;
        self.visible_seconds = DEFAULT_VISIBLE_SECONDS;
        self.user_navigation_active = false;
        self.playhead_offscreen_direction = PlayheadOffscreenDirection::Visible;
        self.clamp_scroll();
    }

    fn restore_view(&mut self, ui_state: &Value) {
        let restored_pixels_per_second = ui_state
            .get("timeline")
            .and_then(|timeline| timeline.get("pixels_per_second"))
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
            .unwrap_or(DEFAULT_PIXELS_PER_SECOND);
        self.pixels_per_second = PixelsPerSecond::clamped(
            restored_pixels_per_second,
            TimelineZoomBounds::default_for_duration(TimelineSeconds::new(self.duration_seconds)),
        )
        .value();
        self.scroll_seconds = ui_state
            .get("timeline")
            .and_then(|timeline| timeline.get("scroll_seconds"))
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
            .map(finite_non_negative)
            .unwrap_or(0.0);
        self.visible_seconds = DEFAULT_VISIBLE_SECONDS;
        self.follow_mode = ui_state
            .get("timeline")
            .and_then(|timeline| timeline.get("follow_mode"))
            .and_then(Value::as_i64)
            .map(|value| TimelineFollowMode::from_i32(value as i32))
            .unwrap_or(TimelineFollowMode::Center);
    }

    fn capture_ui_state(&self, selected_track_id: &str) -> Value {
        json!({
            "selected_track_id": selected_track_id,
            "pixels_per_second": self.pixels_per_second,
            "scroll_seconds": self.scroll_seconds,
            "follow_mode": self.follow_mode.as_i32(),
        })
    }

    fn clamp_scroll(&mut self) {
        self.apply_viewport(self.viewport());
    }

    fn keep_time_visible(&mut self, seconds: f64) {
        let visible_seconds = self.visible_seconds.max(0.01);
        if seconds < self.scroll_seconds {
            self.scroll_seconds = seconds;
        } else if seconds > self.scroll_seconds + visible_seconds {
            self.scroll_seconds = seconds - visible_seconds;
        }
        self.clamp_scroll();
    }

    fn scroll_by_pixels(&mut self, pixel_delta_x: f64) {
        self.user_navigation_active = true;
        let viewport = scroll_by_pixels(self.viewport(), TimelinePixels::new(pixel_delta_x));
        self.apply_viewport(viewport);
    }

    fn zoom_by_factor(
        &mut self,
        factor: f64,
        anchor_x: f64,
        lane_width: f64,
        playhead_seconds: f64,
    ) {
        self.user_navigation_active = true;
        let lane_width = TimelinePixels::non_negative(lane_width);
        let anchor = zoom_anchor_for_pointer_or_playhead(
            self.viewport(),
            TimelinePixels::new(anchor_x),
            lane_width,
            TimelineSeconds::new(playhead_seconds),
            TimelinePixels::new(PLAYHEAD_ANCHOR_THRESHOLD_PIXELS),
        );
        let viewport = zoom_by_factor(self.viewport(), factor, anchor);
        self.apply_viewport(viewport);
    }

    fn set_zoom_for_lane_width(&mut self, pixels_per_second: f64, lane_width: f64) {
        let lane_width = TimelinePixels::non_negative(lane_width);
        self.lane_width_pixels = lane_width.value();
        let bounds =
            TimelineZoomBounds::for_lane(TimelineSeconds::new(self.duration_seconds), lane_width);
        let pixels_per_second = PixelsPerSecond::clamped(pixels_per_second, bounds);
        let viewport = self
            .viewport()
            .with_pixels_per_second(pixels_per_second, Some(lane_width));
        self.apply_viewport(viewport);
    }

    fn fit_to_lane_width(&mut self, lane_width: f64) {
        let lane_width = TimelinePixels::non_negative(lane_width);
        self.lane_width_pixels = lane_width.value();
        let bounds =
            TimelineZoomBounds::for_lane(TimelineSeconds::new(self.duration_seconds), lane_width);
        let viewport = self
            .viewport()
            .with_pixels_per_second(bounds.minimum(), Some(lane_width))
            .with_scroll(TimelineSeconds::new(0.0));
        self.apply_viewport(viewport);
    }

    fn set_visible_lane_width(&mut self, lane_width: f64) {
        let lane_width = TimelinePixels::non_negative(lane_width);
        self.lane_width_pixels = lane_width.value();
        let visible =
            visible_seconds_for_lane(lane_width, PixelsPerSecond::new(self.pixels_per_second));
        let viewport = self.viewport().with_visible(visible);
        self.apply_viewport(viewport);
    }

    fn scrub_at_x(&self, x: f64) -> TimelineSeconds {
        scrub_at_x(self.viewport(), TimelinePixels::new(x))
    }

    fn begin_user_navigation(&mut self) {
        self.user_navigation_active = true;
    }

    fn end_user_navigation(&mut self) {
        self.user_navigation_active = false;
    }

    fn set_follow_mode(&mut self, mode: TimelineFollowMode) {
        self.follow_mode = mode;
    }

    fn apply_follow(&mut self, position_seconds: f64) {
        let result = apply_follow(
            self.viewport(),
            TimelineSeconds::new(position_seconds),
            self.follow_mode,
            self.user_navigation_active,
        );
        self.apply_viewport(result.viewport());
        self.playhead_offscreen_direction = result.offscreen_direction();
    }
}

impl AppControllerState {
    fn sync_timeline_bridge_state_for_current_playhead(&mut self) {
        self.timeline
            .refresh_playhead_offscreen_direction(self.playback_position_seconds);
        self.sync_timeline_bridge_state();
    }

    pub(super) fn sync_timeline_bridge_state(&mut self) {
        self.timeline_duration_seconds = self.timeline.duration_seconds();
        self.timeline_pixels_per_second = self.timeline.pixels_per_second();
        self.timeline_min_pixels_per_second = self.timeline.min_pixels_per_second();
        self.timeline_max_pixels_per_second = self.timeline.max_pixels_per_second();
        self.timeline_scroll_seconds = self.timeline.scroll_seconds();
        self.timeline_visible_seconds = self.timeline.visible_seconds();
        self.timeline_follow_mode = self.timeline.follow_mode().as_i32();
        self.timeline_user_navigation_active = self.timeline.user_navigation_active();
        self.timeline_playhead_offscreen_direction =
            self.timeline.playhead_offscreen_direction().as_i32();
        self.visible_track_range = self.timeline.visible_track_range();
        self.visible_track_ids = self.timeline.visible_track_ids();
    }

    pub(super) fn set_timeline_zoom_state(&mut self, pixels_per_second: f64) {
        self.timeline.set_zoom(pixels_per_second);
        self.sync_timeline_bridge_state_for_current_playhead();
    }

    pub(super) fn set_timeline_scroll_seconds_state(&mut self, seconds: f64) {
        self.timeline.set_scroll_seconds(seconds);
        self.sync_timeline_bridge_state_for_current_playhead();
    }

    pub(super) fn set_timeline_visible_seconds_state(&mut self, seconds: f64) {
        self.timeline.set_visible_seconds(seconds);
        self.sync_timeline_bridge_state_for_current_playhead();
    }

    pub(super) fn set_timeline_visible_lane_width_state(&mut self, lane_width: f64) {
        self.timeline.set_visible_lane_width(lane_width);
        self.sync_timeline_bridge_state_for_current_playhead();
    }

    pub(super) fn set_timeline_zoom_for_lane_width_state(
        &mut self,
        pixels_per_second: f64,
        lane_width: f64,
    ) {
        self.timeline
            .set_zoom_for_lane_width(pixels_per_second, lane_width);
        self.sync_timeline_bridge_state_for_current_playhead();
    }

    pub(super) fn fit_timeline_to_lane_width_state(&mut self, lane_width: f64) {
        self.timeline.fit_to_lane_width(lane_width);
        self.sync_timeline_bridge_state_for_current_playhead();
    }

    pub(super) fn scroll_timeline_by_pixels_state(&mut self, pixel_delta_x: f64) {
        self.timeline.scroll_by_pixels(pixel_delta_x);
        self.sync_timeline_bridge_state_for_current_playhead();
    }

    pub(super) fn zoom_timeline_by_factor_state(
        &mut self,
        factor: f64,
        anchor_x: f64,
        lane_width: f64,
    ) {
        self.timeline
            .zoom_by_factor(factor, anchor_x, lane_width, self.playback_position_seconds);
        self.sync_timeline_bridge_state_for_current_playhead();
    }

    pub(super) fn begin_timeline_user_navigation_state(&mut self) {
        self.timeline.begin_user_navigation();
        self.sync_timeline_bridge_state();
    }

    pub(super) fn end_timeline_user_navigation_state(&mut self) {
        self.timeline.end_user_navigation();
        self.sync_timeline_bridge_state();
    }

    pub(super) fn scrub_timeline_at_x_state(&mut self, x: f64, _lane_width: f64) -> f64 {
        let seconds = self.timeline.scrub_at_x(x);
        self.seek_timeline_position_state(seconds.value());
        seconds.value()
    }

    pub(super) fn set_timeline_follow_mode_state(&mut self, mode: i32) {
        self.timeline
            .set_follow_mode(TimelineFollowMode::from_i32(mode));
        self.sync_timeline_bridge_state();
    }

    pub(super) fn apply_timeline_follow_state(&mut self, position_seconds: f64) {
        self.timeline.apply_follow(position_seconds);
        self.sync_timeline_bridge_state();
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
        let timeline_rows = timeline_rows_for_project_with_state(
            &self.project,
            &self.expanded_track_ids,
            &selected_marker_ids,
        );
        match serde_json::to_string(&timeline_rows) {
            Ok(rows_json) => {
                self.timeline_rows_json = QString::from(&rows_json);
            }
            Err(error) => {
                self.set_error(error.to_string());
            }
        }
        let duration_seconds = self.project_timeline_duration_seconds();
        let scene_snapshot = scene_snapshot_from_project_rows(
            &self.project,
            &timeline_rows,
            duration_seconds,
            &self.selected_track_id.to_string(),
        );
        match serde_json::to_string(&scene_snapshot) {
            Ok(snapshot_json) => {
                self.timeline_scene_snapshot_json = QString::from(&snapshot_json);
            }
            Err(error) => {
                self.set_error(error.to_string());
            }
        }
        self.transform_specs_json = QString::from(
            &transform_specs_json(&self.transform_registry).unwrap_or_else(|_| "[]".to_string()),
        );
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
