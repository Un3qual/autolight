from __future__ import annotations

import math
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from autolight.project.models import ProjectDocument


SNAP_THRESHOLD_PIXELS = 8.0


class MarkerEditingService:
    def snap_time(
        self,
        project: ProjectDocument,
        *,
        requested_seconds: float,
        pixels_per_second: float,
        visible_track_ids: list[str],
        bypass: bool,
    ) -> float:
        requested = self._finite_non_negative(requested_seconds)
        if bypass:
            return requested
        zoom = max(1.0, self._finite_positive(pixels_per_second, fallback=96.0))
        threshold_seconds = SNAP_THRESHOLD_PIXELS / zoom
        eligible_track_ids = self._eligible_snap_track_ids(project, set(visible_track_ids))
        candidates = [
            timestamp
            for marker in project.markers
            if marker.track_id in eligible_track_ids
            and marker.category == "timing"
            for timestamp in [self._finite_candidate_time(marker.timestamp)]
            if timestamp is not None
        ]
        if not candidates:
            return requested
        best = min(candidates, key=lambda value: abs(value - requested))
        return best if abs(best - requested) <= threshold_seconds else requested

    def _eligible_snap_track_ids(self, project: ProjectDocument, visible_track_ids: set[str]) -> set[str]:
        return {
            track.id
            for track in project.tracks
            if track.id in visible_track_ids
            and self._enum_value(track.type) == "generated"
            and self._enum_value(track.result_state) in {"complete", "stale"}
        }

    @staticmethod
    def _finite_non_negative(value: float) -> float:
        number = float(value)
        return number if math.isfinite(number) and number >= 0.0 else 0.0

    @staticmethod
    def _finite_positive(value: float, *, fallback: float) -> float:
        number = float(value)
        return number if math.isfinite(number) and number > 0.0 else fallback

    @staticmethod
    def _finite_candidate_time(value: float) -> float | None:
        try:
            number = float(value)
        except (TypeError, ValueError):
            return None
        return number if math.isfinite(number) else None

    @staticmethod
    def _enum_value(value: object) -> object:
        return getattr(value, "value", value)
