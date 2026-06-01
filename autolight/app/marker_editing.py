from __future__ import annotations

import math

from autolight.project.models import ProjectDocument, ResultState


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
        visible = set(visible_track_ids)
        candidates = [
            marker.timestamp
            for marker in project.markers
            if marker.track_id in visible and self._track_can_snap(project, marker.track_id)
        ]
        if not candidates:
            return requested
        best = min(candidates, key=lambda value: abs(value - requested))
        return best if abs(best - requested) <= threshold_seconds else requested

    def _track_can_snap(self, project: ProjectDocument, track_id: str) -> bool:
        track = next((item for item in project.tracks if item.id == track_id), None)
        if track is None:
            return False
        return track.type.value == "generated" and track.result_state in {
            ResultState.COMPLETE,
            ResultState.STALE,
        }

    @staticmethod
    def _finite_non_negative(value: float) -> float:
        number = float(value)
        return number if math.isfinite(number) and number >= 0.0 else 0.0

    @staticmethod
    def _finite_positive(value: float, *, fallback: float) -> float:
        number = float(value)
        return number if math.isfinite(number) and number > 0.0 else fallback
