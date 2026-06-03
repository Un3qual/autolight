from __future__ import annotations

import math
from typing import Any


class AnalysisLodStore:
    @staticmethod
    def visible_frames(
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
        kind = str(payload.get("kind") or "")
        visible = []
        preceding_frame: dict[str, Any] | None = None
        preceding_time: float | None = None
        for frame in frames:
            if not isinstance(frame, dict):
                continue
            frame_time = _optional_finite_float(frame.get("time"))
            if frame_time is None:
                continue
            if frame_time < start:
                if preceding_time is None or frame_time >= preceding_time:
                    preceding_frame = dict(frame)
                    preceding_time = frame_time
                continue
            if frame_time > stop:
                continue
            visible.append(dict(frame))
        if kind == "harmonic-color" and preceding_frame is not None:
            preceding_frame["time"] = start
            visible.insert(0, preceding_frame)
        if len(visible) > max_frames:
            stride = max(1, math.ceil(len(visible) / max_frames))
            visible = visible[::stride][:max_frames]
        return {
            "kind": kind,
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
