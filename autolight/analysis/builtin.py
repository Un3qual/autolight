from __future__ import annotations

import json
import time
from pathlib import Path

from autolight.analysis.registry import TransformContext, TransformRegistry, TransformResult, TransformSpec


def register_builtin_transforms(registry: TransformRegistry) -> None:
    registry.register(
        TransformSpec(
            id="markers.fixed_interval",
            version="1",
            name="Fixed Interval Markers",
            input_schema="audio-or-markers.v1",
            output_schema="markers.v1",
            estimated_cost="light",
            run=_fixed_interval_markers,
        )
    )
    registry.register(
        TransformSpec(
            id="stems.vocals_stand_in",
            version="1",
            name="Vocals Stem Stand-In",
            input_schema="audio.v1",
            output_schema="artifact.stem.v1",
            estimated_cost="heavy",
            run=_vocals_stand_in,
        )
    )


def _fixed_interval_markers(context: TransformContext, params: dict) -> TransformResult:
    duration = float(params.get("duration", 0.0))
    interval = float(params.get("interval", 1.0))
    if interval <= 0:
        raise ValueError("interval must be greater than zero")

    markers = []
    current = 0.0
    while current <= duration + 1e-9:
        if context.cancel_requested():
            raise RuntimeError("cancelled")
        markers.append(
            {
                "timestamp": round(current, 6),
                "label": "Beat",
                "category": "timing",
                "confidence": 1.0,
                "metadata": {"interval": interval},
            }
        )
        current += interval
    context.progress(1.0)
    return TransformResult(markers=markers)


def _vocals_stand_in(context: TransformContext, params: dict) -> TransformResult:
    label = str(params.get("label", "vocals"))
    context.artifact_dir.mkdir(parents=True, exist_ok=True)
    for step in range(1, 4):
        if context.cancel_requested():
            raise RuntimeError("cancelled")
        context.progress(step / 4)
        time.sleep(0.01)

    artifact = Path(context.artifact_dir) / f"{label}.json"
    artifact.write_text(json.dumps({"stem": label, "samples": []}, sort_keys=True), encoding="utf-8")
    context.progress(1.0)
    return TransformResult(artifacts={"stem": str(artifact)}, metadata={"stem": label})
