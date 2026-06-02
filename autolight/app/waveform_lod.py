from __future__ import annotations

import math
from typing import Any


TARGET_PIXELS_PER_BUCKET = 4.0


class WaveformLodStore:
    def visible_samples(
        self,
        payload: dict[str, Any],
        *,
        scroll_seconds: float,
        visible_seconds: float,
        pixels_per_second: float,
    ) -> dict[str, Any]:
        duration = self._duration(payload)
        levels = self._levels(payload)
        if not levels:
            return {"duration": duration, "level_bucket_count": 0, "samples": []}
        level = self._select_level(
            levels,
            duration=duration,
            visible_seconds=visible_seconds,
            pixels_per_second=pixels_per_second,
        )
        bucket_count = int(level["bucket_count"])
        samples = list(level["samples"])
        if duration <= 0.0 or bucket_count <= 0:
            return {"duration": duration, "level_bucket_count": bucket_count, "samples": []}
        start_seconds = max(0.0, self._finite_float(scroll_seconds, default=0.0))
        stop_seconds = min(
            duration,
            start_seconds + max(0.01, self._finite_float(visible_seconds, default=0.01)),
        )
        start_index = max(0, math.floor(start_seconds / duration * bucket_count) - 1)
        stop_index = min(bucket_count, math.ceil(stop_seconds / duration * bucket_count) + 1)
        visible = [
            {
                **self._sample_dict(sample),
                "time": (index / max(1, bucket_count)) * duration,
            }
            for index, sample in enumerate(samples[start_index:stop_index], start=start_index)
        ]
        return {
            "duration": duration,
            "level_bucket_count": bucket_count,
            "samples": visible,
        }

    def _select_level(
        self,
        levels: list[dict[str, Any]],
        *,
        duration: float,
        visible_seconds: float,
        pixels_per_second: float,
    ) -> dict[str, Any]:
        visible = max(0.01, self._finite_float(visible_seconds, default=0.01))
        zoom = max(1.0, self._finite_float(pixels_per_second, default=1.0))
        desired = max(1, math.ceil(visible * zoom / TARGET_PIXELS_PER_BUCKET))
        if duration > 0.0:
            visible_fraction = visible / duration
            return min(
                levels,
                key=lambda level: abs(int(level["bucket_count"]) * visible_fraction - desired),
            )
        return min(levels, key=lambda level: abs(int(level["bucket_count"]) - desired))

    def _levels(self, payload: dict[str, Any]) -> list[dict[str, Any]]:
        levels = payload.get("levels")
        if isinstance(levels, list) and levels:
            parsed_levels = [
                level
                for level in (self._parse_level(level) for level in levels)
                if level is not None
            ]
            if parsed_levels:
                return parsed_levels
        samples = payload.get("samples", [])
        return [
            {"bucket_count": len(samples), "samples": list(samples)}
        ] if isinstance(samples, list) and samples else []

    def _duration(self, payload: dict[str, Any]) -> float:
        duration = self._finite_float(payload.get("duration", 0.0), default=0.0)
        return duration if duration >= 0.0 else 0.0

    def _parse_level(self, level: Any) -> dict[str, Any] | None:
        if not isinstance(level, dict):
            return None
        samples = level.get("samples", [])
        if not isinstance(samples, list):
            return None
        sample_list = list(samples)
        if not sample_list:
            return None
        sample_count = len(sample_list)
        bucket_count = self._bucket_count(level.get("bucket_count"), default=sample_count)
        if bucket_count != sample_count:
            bucket_count = sample_count
        return {"bucket_count": bucket_count, "samples": sample_list}

    @staticmethod
    def _finite_float(value: Any, *, default: float) -> float:
        try:
            result = float(value)
        except (OverflowError, TypeError, ValueError):
            return default
        return result if math.isfinite(result) else default

    @staticmethod
    def _bucket_count(value: Any, *, default: int) -> int:
        try:
            result = int(value)
        except (OverflowError, TypeError, ValueError):
            return default
        return result if result > 0 else default

    @staticmethod
    def _sample_dict(sample: Any) -> dict[str, Any]:
        return dict(sample) if isinstance(sample, dict) else {}
