from __future__ import annotations

import json
import math
import shutil
import time
from pathlib import Path

from autolight.analysis.music import MusicAnalysisEngine
from autolight.analysis.registry import (
    TransformCancelled,
    TransformContext,
    TransformRegistry,
    TransformResult,
    TransformSpec,
)
from autolight.analysis.timing import detect_beat_markers, detect_onset_markers
from autolight.analysis.waveform import MAX_WAVEFORM_LOD_BUCKETS, build_waveform_summary

MAX_FIXED_INTERVAL_MARKERS = 100_000
MAX_WAVEFORM_BUCKETS = MAX_WAVEFORM_LOD_BUCKETS


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
    registry.register(
        TransformSpec(
            id="audio.drums_stand_in",
            version="1",
            name="Drums Stem Stand-In",
            input_schema="audio.v1",
            output_schema="artifact.audio.v1",
            estimated_cost="medium",
            run=_drums_stand_in,
        )
    )
    registry.register(
        TransformSpec(
            id="timing.onsets",
            version="1",
            name="Onsets",
            input_schema="audio.v1",
            output_schema="markers.v1",
            estimated_cost="medium",
            run=_timing_onsets,
        )
    )
    registry.register(
        TransformSpec(
            id="timing.beats",
            version="1",
            name="Beats",
            input_schema="audio.v1",
            output_schema="markers.v1",
            estimated_cost="medium",
            run=_timing_beats,
        )
    )
    registry.register(
        TransformSpec(
            id="waveform.summary",
            version="1",
            name="Waveform Summary",
            input_schema="audio.v1",
            output_schema="artifact.waveform.v1",
            estimated_cost="medium",
            run=_waveform_summary,
        )
    )
    registry.register(
        TransformSpec(
            id="music.beat_grid",
            version="1",
            name="Beat Grid",
            input_schema="audio.v1",
            output_schema="artifact.beat-grid.v1",
            estimated_cost="medium",
            run=_music_beat_grid,
        )
    )
    registry.register(
        TransformSpec(
            id="music.energy_profile",
            version="1",
            name="Energy Profile",
            input_schema="audio.v1",
            output_schema="artifact.energy.v1",
            estimated_cost="medium",
            run=_music_energy_profile,
        )
    )
    registry.register(
        TransformSpec(
            id="music.harmonic_color",
            version="1",
            name="Harmonic Color",
            input_schema="audio.v1",
            output_schema="artifact.harmonic-color.v1",
            estimated_cost="medium",
            run=_music_harmonic_color,
        )
    )


def _fixed_interval_markers(context: TransformContext, params: dict) -> TransformResult:
    duration = float(params.get("duration", 0.0))
    interval = float(params.get("interval", 1.0))
    if not math.isfinite(duration) or not math.isfinite(interval):
        raise ValueError("duration and interval must be finite")
    if interval <= 0:
        raise ValueError("interval must be greater than zero")
    if duration < 0:
        raise ValueError("duration must be greater than or equal to zero")

    marker_count = math.floor((duration + 1e-9) / interval) + 1
    if marker_count > MAX_FIXED_INTERVAL_MARKERS:
        raise ValueError(f"too many markers requested: {marker_count}")

    markers = []
    current = 0.0
    while current <= duration + 1e-9:
        if context.cancel_requested():
            raise TransformCancelled("cancelled")
        markers.append(
            {
                "timestamp": round(current, 6),
                "label": "Beat",
                "category": "timing",
                "confidence": 1.0,
                "metadata": {"interval": interval},
            }
        )
        next_current = current + interval
        if duration > 0 and next_current < duration - 1e-9:
            context.progress(min(max(next_current / duration, 0.0), 1.0))
        current += interval
    context.progress(1.0)
    return TransformResult(markers=markers)


def _vocals_stand_in(context: TransformContext, params: dict) -> TransformResult:
    label = str(params.get("label", "vocals"))
    context.artifact_dir.mkdir(parents=True, exist_ok=True)
    for step in range(1, 4):
        if context.cancel_requested():
            raise TransformCancelled("cancelled")
        context.progress(step / 4)
        time.sleep(0.01)

    if context.cancel_requested():
        raise TransformCancelled("cancelled")

    audio_path = params.get("audio_path")
    if audio_path:
        artifact = Path(context.artifact_dir) / "stem.wav"
        shutil.copyfile(Path(str(audio_path)), artifact)
        context.progress(1.0)
        return TransformResult(artifacts={"stem": str(artifact)}, metadata={"stem": label})

    artifact = Path(context.artifact_dir) / "stem.json"
    artifact.resolve().relative_to(Path(context.artifact_dir).resolve())
    artifact.write_text(json.dumps({"stem": label, "samples": []}, sort_keys=True), encoding="utf-8")
    context.progress(1.0)
    return TransformResult(artifacts={"stem": str(artifact)}, metadata={"stem": label})


def _drums_stand_in(context: TransformContext, params: dict) -> TransformResult:
    source = Path(str(params["audio_path"]))
    context.artifact_dir.mkdir(parents=True, exist_ok=True)
    _raise_if_cancelled(context)
    output = Path(context.artifact_dir) / "drums.wav"
    shutil.copyfile(source, output)
    context.progress(1.0)
    return TransformResult(artifacts={"audio": str(output)}, metadata={"stem": "drums"})


def _timing_onsets(context: TransformContext, params: dict) -> TransformResult:
    _raise_if_cancelled(context)
    context.progress(0.1)
    markers = detect_onset_markers(Path(str(params["audio_path"])))
    _raise_if_cancelled(context)
    context.progress(1.0)
    return TransformResult(markers=markers)


def _timing_beats(context: TransformContext, params: dict) -> TransformResult:
    _raise_if_cancelled(context)
    context.progress(0.1)
    markers = detect_beat_markers(Path(str(params["audio_path"])))
    _raise_if_cancelled(context)
    context.progress(1.0)
    return TransformResult(markers=markers)


def _waveform_summary(context: TransformContext, params: dict) -> TransformResult:
    audio_path = Path(str(params["audio_path"]))
    buckets = min(int(params.get("buckets", 512)), MAX_WAVEFORM_BUCKETS)
    context.artifact_dir.mkdir(parents=True, exist_ok=True)
    _raise_if_cancelled(context)
    context.progress(0.1)
    output_path = Path(context.artifact_dir) / "waveform.json"
    build_waveform_summary(
        audio_path,
        output_path,
        buckets=buckets,
        cancel_requested=context.cancel_requested,
    )
    _raise_if_cancelled(context)
    context.progress(1.0)
    return TransformResult(
        artifacts={"waveform": str(output_path)},
        metadata={"bucket_count": buckets},
    )


def _music_beat_grid(context: TransformContext, params: dict) -> TransformResult:
    return _run_music_analysis(context, params, "beat-grid", MusicAnalysisEngine.analyze_rhythm)


def _music_energy_profile(context: TransformContext, params: dict) -> TransformResult:
    return _run_music_analysis(context, params, "energy", MusicAnalysisEngine.analyze_energy)


def _music_harmonic_color(context: TransformContext, params: dict) -> TransformResult:
    return _run_music_analysis(context, params, "harmonic-color", MusicAnalysisEngine.analyze_harmony)


def _run_music_analysis(context: TransformContext, params: dict, artifact_kind: str, analyzer) -> TransformResult:
    _raise_if_cancelled(context)
    context.progress(0.05)
    audio_path = Path(str(params["audio_path"]))
    settings = {key: value for key, value in params.items() if key != "audio_path"}
    result = analyzer(audio_path, settings)
    _raise_if_cancelled(context)
    context.artifact_dir.mkdir(parents=True, exist_ok=True)
    artifact_path = Path(context.artifact_dir) / f"{artifact_kind}.json"
    artifact_path.write_text(json.dumps(result.payload, sort_keys=True), encoding="utf-8")
    context.progress(1.0)
    return TransformResult(
        markers=result.markers,
        artifacts={artifact_kind: str(artifact_path)},
        metadata={"kind": artifact_kind},
    )


def _raise_if_cancelled(context: TransformContext) -> None:
    if context.cancel_requested():
        raise TransformCancelled("cancelled")
