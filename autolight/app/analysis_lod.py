from __future__ import annotations

import math
from typing import Any


class AnalysisLodStore:
    def visible_frames(
        self,
        payload: dict[str, Any],
        *,
        scroll_seconds: float,
        visible_seconds: float,
        max_frames: int = 256,
    ) -> dict[str, Any]:
        frames = payload.get("frames", [])
        if not isinstance(frames, list):
            frames = []
        start = max(0.0, _finite_float(scroll_seconds))
        stop = start + max(0.0, _finite_float(visible_seconds))
        visible = []
        for frame in frames:
            if not isinstance(frame, dict):
                continue
            frame_time = _optional_finite_float(frame.get("time"))
            if frame_time is None or not start <= frame_time <= stop:
                continue
            visible.append(dict(frame))
        if len(visible) > max_frames:
            stride = max(1, math.ceil(len(visible) / max_frames))
            visible = visible[::stride][:max_frames]
        return {
            "kind": str(payload.get("kind", "")),
            "duration": max(0.0, _finite_float(payload.get("duration", 0.0))),
            "frames": visible,
        }


def _finite_float(value) -> float:
    try:
        result = float(value)
    except (TypeError, ValueError, OverflowError):
        return 0.0
    return result if math.isfinite(result) else 0.0


def _optional_finite_float(value) -> float | None:
    try:
        result = float(value)
    except (TypeError, ValueError, OverflowError):
        return None
    return result if math.isfinite(result) else None
