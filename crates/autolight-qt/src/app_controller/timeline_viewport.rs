const MIN_VISIBLE_SECONDS: f64 = 0.01;
const EMPTY_PROJECT_MIN_PIXELS_PER_SECOND: f64 = 8.0;
const MIN_DYNAMIC_PIXELS_PER_SECOND: f64 = 0.1;
pub(super) const MAX_PIXELS_PER_SECOND: f64 = 8_000.0;
pub(super) const DEFAULT_PIXELS_PER_SECOND: f64 = 96.0;
pub(super) const DEFAULT_VISIBLE_SECONDS: f64 = 8.0;
pub(super) const PLAYHEAD_ANCHOR_THRESHOLD_PIXELS: f64 = 18.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TimelineSeconds(f64);

impl TimelineSeconds {
    pub(super) fn new(value: f64) -> Self {
        Self(finite_non_negative(value))
    }

    pub(super) fn value(self) -> f64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TimelinePixels(f64);

impl TimelinePixels {
    pub(super) fn new(value: f64) -> Self {
        Self(if value.is_finite() { value } else { 0.0 })
    }

    pub(super) fn non_negative(value: f64) -> Self {
        Self(finite_non_negative(value))
    }

    pub(super) fn value(self) -> f64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PixelsPerSecond(f64);

impl PixelsPerSecond {
    pub(super) fn new(value: f64) -> Self {
        Self(sanitize_pixels_per_second(value))
    }

    pub(super) fn clamped(value: f64, bounds: TimelineZoomBounds) -> Self {
        let value = sanitize_pixels_per_second(value);
        Self(value.clamp(bounds.minimum.value(), bounds.maximum.value()))
    }

    pub(super) fn value(self) -> f64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TimelineZoomBounds {
    minimum: PixelsPerSecond,
    maximum: PixelsPerSecond,
}

impl TimelineZoomBounds {
    pub(super) fn for_lane(duration: TimelineSeconds, lane_width: TimelinePixels) -> Self {
        let duration = duration.value();
        let lane_width = lane_width.value();
        let minimum = if duration > 0.0 && lane_width > 0.0 {
            (lane_width / duration).clamp(MIN_DYNAMIC_PIXELS_PER_SECOND, MAX_PIXELS_PER_SECOND)
        } else {
            EMPTY_PROJECT_MIN_PIXELS_PER_SECOND
        };
        Self {
            minimum: PixelsPerSecond(minimum),
            maximum: PixelsPerSecond(MAX_PIXELS_PER_SECOND),
        }
    }

    pub(super) fn default_for_duration(duration: TimelineSeconds) -> Self {
        Self::for_lane(duration, TimelinePixels::non_negative(0.0))
    }

    pub(super) fn minimum(self) -> PixelsPerSecond {
        self.minimum
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TimelineViewport {
    duration: TimelineSeconds,
    pixels_per_second: PixelsPerSecond,
    scroll: TimelineSeconds,
    visible: TimelineSeconds,
}

impl TimelineViewport {
    pub(super) fn new(
        duration: f64,
        pixels_per_second: f64,
        scroll_seconds: f64,
        visible_seconds: f64,
    ) -> Self {
        let duration = TimelineSeconds::new(duration);
        let pixels_per_second = PixelsPerSecond::new(pixels_per_second);
        let visible = TimelineSeconds::new(visible_seconds.max(MIN_VISIBLE_SECONDS));
        let scroll = clamp_scroll_seconds(TimelineSeconds::new(scroll_seconds), duration, visible);
        Self {
            duration,
            pixels_per_second,
            scroll,
            visible,
        }
    }

    pub(super) fn duration(self) -> TimelineSeconds {
        self.duration
    }

    pub(super) fn pixels_per_second(self) -> PixelsPerSecond {
        self.pixels_per_second
    }

    pub(super) fn scroll(self) -> TimelineSeconds {
        self.scroll
    }

    pub(super) fn visible(self) -> TimelineSeconds {
        self.visible
    }

    pub(super) fn with_pixels_per_second(
        self,
        pixels_per_second: PixelsPerSecond,
        lane_width: Option<TimelinePixels>,
    ) -> Self {
        let visible = lane_width
            .map(|width| visible_seconds_for_lane(width, pixels_per_second))
            .unwrap_or(self.visible);
        Self::new(
            self.duration.value(),
            pixels_per_second.value(),
            self.scroll.value(),
            visible.value(),
        )
    }

    pub(super) fn with_scroll(self, scroll: TimelineSeconds) -> Self {
        Self::new(
            self.duration.value(),
            self.pixels_per_second.value(),
            scroll.value(),
            self.visible.value(),
        )
    }

    pub(super) fn with_visible(self, visible: TimelineSeconds) -> Self {
        Self::new(
            self.duration.value(),
            self.pixels_per_second.value(),
            self.scroll.value(),
            visible.value(),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TimelineZoomAnchor {
    x: TimelinePixels,
    seconds: TimelineSeconds,
    lane_width: TimelinePixels,
}

impl TimelineZoomAnchor {
    pub(super) fn new(
        x: TimelinePixels,
        seconds: TimelineSeconds,
        lane_width: TimelinePixels,
    ) -> Self {
        Self {
            x,
            seconds,
            lane_width,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TimelineFollowMode {
    Off,
    Band,
    Center,
}

impl TimelineFollowMode {
    pub(super) fn from_i32(value: i32) -> Self {
        match value {
            0 => Self::Off,
            1 => Self::Band,
            2 => Self::Center,
            _ => Self::Center,
        }
    }

    pub(super) fn as_i32(self) -> i32 {
        match self {
            Self::Off => 0,
            Self::Band => 1,
            Self::Center => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PlayheadOffscreenDirection {
    Left,
    Visible,
    Right,
}

impl PlayheadOffscreenDirection {
    pub(super) fn as_i32(self) -> i32 {
        match self {
            Self::Left => -1,
            Self::Visible => 0,
            Self::Right => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TimelineFollowResult {
    viewport: TimelineViewport,
    offscreen_direction: PlayheadOffscreenDirection,
}

impl TimelineFollowResult {
    pub(super) fn viewport(self) -> TimelineViewport {
        self.viewport
    }

    pub(super) fn offscreen_direction(self) -> PlayheadOffscreenDirection {
        self.offscreen_direction
    }
}

pub(super) fn scroll_by_pixels(
    viewport: TimelineViewport,
    delta: TimelinePixels,
) -> TimelineViewport {
    let seconds_delta = delta.value() / viewport.pixels_per_second.value();
    viewport.with_scroll(TimelineSeconds::new(
        viewport.scroll.value() + seconds_delta,
    ))
}

pub(super) fn zoom_anchor_for_pointer_or_playhead(
    viewport: TimelineViewport,
    pointer_x: TimelinePixels,
    lane_width: TimelinePixels,
    playhead_seconds: TimelineSeconds,
    threshold: TimelinePixels,
) -> TimelineZoomAnchor {
    let playhead_x =
        (playhead_seconds.value() - viewport.scroll.value()) * viewport.pixels_per_second.value();
    if playhead_x.is_finite()
        && playhead_x >= 0.0
        && playhead_x <= lane_width.value()
        && (pointer_x.value() - playhead_x).abs() <= threshold.value().abs()
    {
        return TimelineZoomAnchor::new(
            TimelinePixels::new(playhead_x),
            playhead_seconds,
            lane_width,
        );
    }
    TimelineZoomAnchor::new(pointer_x, scrub_at_x(viewport, pointer_x), lane_width)
}

pub(super) fn zoom_by_factor(
    viewport: TimelineViewport,
    factor: f64,
    anchor: TimelineZoomAnchor,
) -> TimelineViewport {
    if !factor.is_finite() || factor <= 0.0 {
        return viewport;
    }
    let bounds = TimelineZoomBounds::for_lane(viewport.duration, anchor.lane_width);
    let next_pixels_per_second =
        PixelsPerSecond::clamped(viewport.pixels_per_second.value() * factor, bounds);
    let next_visible = visible_seconds_for_lane(anchor.lane_width, next_pixels_per_second);
    let next_scroll = anchor.seconds.value() - anchor.x.value() / next_pixels_per_second.value();
    TimelineViewport::new(
        viewport.duration.value(),
        next_pixels_per_second.value(),
        next_scroll,
        next_visible.value(),
    )
}

pub(super) fn scrub_at_x(viewport: TimelineViewport, x: TimelinePixels) -> TimelineSeconds {
    let seconds = viewport.scroll.value() + x.value().max(0.0) / viewport.pixels_per_second.value();
    TimelineSeconds::new(seconds.min(viewport.duration.value()))
}

pub(super) fn apply_follow(
    viewport: TimelineViewport,
    position: TimelineSeconds,
    mode: TimelineFollowMode,
    user_navigation_active: bool,
) -> TimelineFollowResult {
    let offscreen_direction = playhead_offscreen_direction_for(viewport, position);
    if user_navigation_active || mode == TimelineFollowMode::Off {
        return TimelineFollowResult {
            viewport,
            offscreen_direction,
        };
    }
    let visible = viewport.visible.value().max(MIN_VISIBLE_SECONDS);
    let edge_band = (visible * 0.18).clamp(0.25, 2.0).min(visible / 2.0);
    let target_scroll = match mode {
        TimelineFollowMode::Off => viewport.scroll.value(),
        TimelineFollowMode::Band => {
            let left_edge = viewport.scroll.value() + edge_band;
            let right_edge = viewport.scroll.value() + visible - edge_band;
            if position.value() < left_edge {
                position.value() - edge_band
            } else if position.value() > right_edge {
                position.value() - visible + edge_band
            } else {
                viewport.scroll.value()
            }
        }
        TimelineFollowMode::Center => {
            let center = viewport.scroll.value() + visible / 2.0;
            if position.value() >= center || position.value() < viewport.scroll.value() {
                position.value() - visible / 2.0
            } else {
                viewport.scroll.value()
            }
        }
    };
    let viewport = viewport.with_scroll(TimelineSeconds::new(target_scroll));
    TimelineFollowResult {
        viewport,
        offscreen_direction: playhead_offscreen_direction_for(viewport, position),
    }
}

pub(super) fn playhead_offscreen_direction_for(
    viewport: TimelineViewport,
    position: TimelineSeconds,
) -> PlayheadOffscreenDirection {
    if position.value() < viewport.scroll.value() {
        PlayheadOffscreenDirection::Left
    } else if position.value() > viewport.scroll.value() + viewport.visible.value() {
        PlayheadOffscreenDirection::Right
    } else {
        PlayheadOffscreenDirection::Visible
    }
}

pub(super) fn visible_seconds_for_lane(
    lane_width: TimelinePixels,
    pixels_per_second: PixelsPerSecond,
) -> TimelineSeconds {
    TimelineSeconds::new((lane_width.value() / pixels_per_second.value()).max(MIN_VISIBLE_SECONDS))
}

fn clamp_scroll_seconds(
    scroll: TimelineSeconds,
    duration: TimelineSeconds,
    visible: TimelineSeconds,
) -> TimelineSeconds {
    TimelineSeconds::new(
        scroll
            .value()
            .clamp(0.0, max_scroll_seconds(duration, visible)),
    )
}

fn max_scroll_seconds(duration: TimelineSeconds, visible: TimelineSeconds) -> f64 {
    (duration.value() - visible.value()).max(0.0)
}

fn finite_non_negative(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn sanitize_pixels_per_second(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value.clamp(MIN_DYNAMIC_PIXELS_PER_SECOND, MAX_PIXELS_PER_SECOND)
    } else {
        DEFAULT_PIXELS_PER_SECOND
    }
}

#[cfg(test)]
mod tests {
    use super::{
        apply_follow, scroll_by_pixels, scrub_at_x, zoom_anchor_for_pointer_or_playhead,
        zoom_by_factor, PixelsPerSecond, PlayheadOffscreenDirection, TimelineFollowMode,
        TimelinePixels, TimelineSeconds, TimelineViewport, TimelineZoomAnchor,
    };

    #[test]
    fn scroll_by_pixels_converts_delta_through_pixels_per_second_and_clamps() {
        let viewport = TimelineViewport::new(12.0, 100.0, 2.0, 4.0);

        let scrolled = scroll_by_pixels(viewport, TimelinePixels::new(150.0));

        assert_eq!(scrolled.scroll().value(), 3.5);

        let clamped = scroll_by_pixels(scrolled, TimelinePixels::new(10_000.0));

        assert_eq!(clamped.scroll().value(), 8.0);

        let lower_clamped = scroll_by_pixels(scrolled, TimelinePixels::new(-1_000.0));

        assert_eq!(lower_clamped.scroll().value(), 0.0);
    }

    #[test]
    fn zoom_by_factor_preserves_anchor_screen_x_after_zoom() {
        let viewport = TimelineViewport::new(20.0, 100.0, 2.0, 5.0);
        let anchor = TimelineZoomAnchor::new(
            TimelinePixels::new(200.0),
            TimelineSeconds::new(4.0),
            TimelinePixels::new(500.0),
        );

        let zoomed = zoom_by_factor(viewport, 2.0, anchor);
        let anchor_x_after_zoom =
            (anchor.seconds.value() - zoomed.scroll().value()) * zoomed.pixels_per_second().value();

        assert_eq!(anchor_x_after_zoom, 200.0);
    }

    #[test]
    fn zoom_by_factor_clamps_to_dynamic_minimum_and_maximum() {
        let viewport = TimelineViewport::new(100.0, 100.0, 10.0, 5.0);
        let anchor = TimelineZoomAnchor::new(
            TimelinePixels::new(250.0),
            TimelineSeconds::new(12.5),
            TimelinePixels::new(500.0),
        );

        let zoomed_out = zoom_by_factor(viewport, 0.0001, anchor);
        let zoomed_in = zoom_by_factor(viewport, 100_000.0, anchor);

        assert_eq!(zoomed_out.pixels_per_second().value(), 5.0);
        assert_eq!(
            zoomed_in.pixels_per_second().value(),
            super::MAX_PIXELS_PER_SECOND
        );
    }

    #[test]
    fn zoom_anchor_prefers_playhead_when_pointer_is_near_playhead() {
        let viewport = TimelineViewport::new(20.0, 100.0, 2.0, 5.0);

        let anchor = zoom_anchor_for_pointer_or_playhead(
            viewport,
            TimelinePixels::new(305.0),
            TimelinePixels::new(500.0),
            TimelineSeconds::new(5.0),
            TimelinePixels::new(18.0),
        );

        assert_eq!(anchor.seconds.value(), 5.0);
        assert_eq!(anchor.x.value(), 300.0);
    }

    #[test]
    fn zoom_anchor_uses_pointer_time_when_pointer_is_not_near_playhead() {
        let viewport = TimelineViewport::new(20.0, 100.0, 2.0, 5.0);

        let anchor = zoom_anchor_for_pointer_or_playhead(
            viewport,
            TimelinePixels::new(100.0),
            TimelinePixels::new(500.0),
            TimelineSeconds::new(5.0),
            TimelinePixels::new(18.0),
        );

        assert_eq!(anchor.seconds.value(), 3.0);
        assert_eq!(anchor.x.value(), 100.0);
    }

    #[test]
    fn policy_sanitizes_non_finite_inputs_without_corrupting_viewport() {
        let viewport = TimelineViewport::new(f64::NAN, f64::INFINITY, -5.0, f64::NAN);

        assert_eq!(viewport.duration().value(), 0.0);
        assert_eq!(
            viewport.pixels_per_second().value(),
            super::DEFAULT_PIXELS_PER_SECOND
        );
        assert_eq!(viewport.scroll().value(), 0.0);
        assert_eq!(viewport.visible().value(), 0.01);

        let scrolled = scroll_by_pixels(viewport, TimelinePixels::new(f64::NAN));
        let zoomed = zoom_by_factor(
            viewport,
            f64::INFINITY,
            TimelineZoomAnchor::new(
                TimelinePixels::new(f64::NAN),
                TimelineSeconds::new(f64::NAN),
                TimelinePixels::new(f64::NAN),
            ),
        );

        assert_eq!(scrolled.scroll().value(), 0.0);
        assert_eq!(
            zoomed.pixels_per_second().value(),
            viewport.pixels_per_second().value()
        );
    }

    #[test]
    fn conversions_respect_pixels_per_second_below_one() {
        let viewport = TimelineViewport::new(10_000.0, 0.5, 0.0, 1_000.0);

        let scrolled = scroll_by_pixels(viewport, TimelinePixels::new(10.0));
        let scrubbed = scrub_at_x(viewport, TimelinePixels::new(10.0));
        let visible =
            super::visible_seconds_for_lane(TimelinePixels::new(10.0), PixelsPerSecond::new(0.5));

        assert_eq!(scrolled.scroll().value(), 20.0);
        assert_eq!(scrubbed.value(), 20.0);
        assert_eq!(visible.value(), 20.0);
    }

    #[test]
    fn scrub_at_x_converts_lane_x_to_clamped_timeline_seconds() {
        let viewport = TimelineViewport::new(8.0, 100.0, 2.0, 4.0);

        assert_eq!(
            scrub_at_x(viewport, TimelinePixels::new(250.0)).value(),
            4.5
        );
        assert_eq!(
            scrub_at_x(viewport, TimelinePixels::new(2_000.0)).value(),
            8.0
        );
    }

    #[test]
    fn follow_center_keeps_playhead_centered_after_it_reaches_center() {
        let viewport = TimelineViewport::new(20.0, 100.0, 0.0, 4.0);

        let followed = apply_follow(
            viewport,
            TimelineSeconds::new(5.0),
            TimelineFollowMode::Center,
            false,
        );

        assert_eq!(followed.viewport().scroll().value(), 3.0);
        assert_eq!(
            followed.offscreen_direction(),
            PlayheadOffscreenDirection::Visible
        );
    }

    #[test]
    fn follow_is_suppressed_while_user_navigation_is_active() {
        let viewport = TimelineViewport::new(20.0, 100.0, 0.0, 4.0);

        let followed = apply_follow(
            viewport,
            TimelineSeconds::new(8.0),
            TimelineFollowMode::Center,
            true,
        );

        assert_eq!(followed.viewport().scroll().value(), 0.0);
        assert_eq!(
            followed.offscreen_direction(),
            PlayheadOffscreenDirection::Right
        );
    }

    #[test]
    fn zoom_bounds_can_fit_project_duration_into_lane_width() {
        let bounds = super::TimelineZoomBounds::for_lane(
            TimelineSeconds::new(100.0),
            TimelinePixels::new(500.0),
        );

        assert_eq!(bounds.minimum(), PixelsPerSecond::new(5.0));
    }
}
