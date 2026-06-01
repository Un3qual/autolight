from __future__ import annotations

import math


TIMELINE_MIN_PIXELS_PER_SECOND = 24.0
TIMELINE_MAX_PIXELS_PER_SECOND = 240.0
FOLLOW_EDGE_FRACTION = 0.20
FOLLOW_MAX_HZ = 30.0


class TimelineViewport:
    def __init__(self):
        self._last_follow_emit_seconds = -math.inf

    def clamp_zoom(self, pixels_per_second: float) -> float:
        value = self._finite_positive(pixels_per_second, fallback=96.0)
        return min(max(value, TIMELINE_MIN_PIXELS_PER_SECOND), TIMELINE_MAX_PIXELS_PER_SECOND)

    def clamp_scroll(
        self,
        scroll_seconds: float,
        *,
        visible_seconds: float,
        duration_seconds: float,
    ) -> float:
        value = self._finite_non_negative(scroll_seconds)
        max_scroll = max(0.0, self._finite_non_negative(duration_seconds) - max(0.01, visible_seconds))
        return min(value, max_scroll)

    def scroll_for_follow(
        self,
        *,
        position_seconds: float,
        scroll_seconds: float,
        visible_seconds: float,
        duration_seconds: float,
    ) -> float:
        visible = max(0.01, self._finite_positive(visible_seconds, fallback=0.01))
        current = self.clamp_scroll(
            scroll_seconds,
            visible_seconds=visible,
            duration_seconds=duration_seconds,
        )
        position = self._finite_non_negative(position_seconds)
        leading_edge = current + visible * FOLLOW_EDGE_FRACTION
        trailing_edge = current + visible * (1.0 - FOLLOW_EDGE_FRACTION)
        if position < leading_edge:
            target = position - visible * FOLLOW_EDGE_FRACTION
        elif position > trailing_edge:
            target = position - visible * (1.0 - FOLLOW_EDGE_FRACTION)
        else:
            target = current
        return self.clamp_scroll(target, visible_seconds=visible, duration_seconds=duration_seconds)

    def should_emit_follow_scroll(self, now_seconds: float) -> bool:
        now = self._finite_non_negative(now_seconds)
        minimum_interval = 1.0 / FOLLOW_MAX_HZ
        if now - self._last_follow_emit_seconds < minimum_interval:
            return False
        self._last_follow_emit_seconds = now
        return True

    def zoom_around_anchor(
        self,
        *,
        current_zoom: float,
        requested_zoom: float,
        current_scroll: float,
        visible_seconds: float,
        duration_seconds: float,
        anchor_seconds: float,
    ) -> tuple[float, float]:
        old_zoom = self.clamp_zoom(current_zoom)
        new_zoom = self.clamp_zoom(requested_zoom)
        visible = max(0.01, self._finite_positive(visible_seconds, fallback=0.01))
        scroll = self.clamp_scroll(
            current_scroll,
            visible_seconds=visible,
            duration_seconds=duration_seconds,
        )
        anchor = self._finite_non_negative(anchor_seconds)
        anchor_fraction = 0.0 if visible <= 0.0 else (anchor - scroll) / visible
        new_visible = visible * old_zoom / new_zoom
        new_scroll = anchor - anchor_fraction * new_visible
        return new_zoom, self.clamp_scroll(
            new_scroll,
            visible_seconds=new_visible,
            duration_seconds=duration_seconds,
        )

    @staticmethod
    def _finite_non_negative(value: float) -> float:
        number = float(value)
        return number if math.isfinite(number) and number >= 0.0 else 0.0

    @staticmethod
    def _finite_positive(value: float, *, fallback: float) -> float:
        number = float(value)
        return number if math.isfinite(number) and number > 0.0 else fallback
