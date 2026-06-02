from __future__ import annotations

import math
import warnings
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import librosa
import numpy as np
from librosa.util.exceptions import ParameterError


DEFAULT_HOP_LENGTH = 512
DEFAULT_MAX_FRAMES = 2048
DEFAULT_MAX_MARKERS = 2048


@dataclass(slots=True)
class MusicAnalysisResult:
    kind: str
    payload: dict[str, Any]
    markers: list[dict[str, Any]] = field(default_factory=list)
    frames: list[dict[str, Any]] = field(default_factory=list)


class MusicAnalysisEngine:
    def analyze_rhythm(self, audio_path: str | Path, settings: dict[str, Any] | None = None) -> MusicAnalysisResult:
        settings = dict(settings or {})
        hop_length = _positive_int(settings.get("hop_length", DEFAULT_HOP_LENGTH), "hop_length")
        max_markers = _positive_int(settings.get("max_markers", DEFAULT_MAX_MARKERS), "max_markers")
        y, sr = _load_audio(audio_path)
        tempo, beat_frames = librosa.beat.beat_track(y=y, sr=sr, hop_length=hop_length, units="frames")
        beat_times = librosa.frames_to_time(beat_frames, sr=sr, hop_length=hop_length)
        onset_env = librosa.onset.onset_strength(y=y, sr=sr, hop_length=hop_length)
        tempo_value = _first_float(tempo)
        markers = []
        for index, timestamp in enumerate(beat_times[:max_markers]):
            beat_strength = _frame_value(onset_env, int(beat_frames[index]) if index < len(beat_frames) else 0)
            category = "downbeat" if index % 4 == 0 else "beat"
            markers.append(
                {
                    "timestamp": round(float(timestamp), 6),
                    "label": "Downbeat" if category == "downbeat" else "Beat",
                    "category": category,
                    "confidence": beat_strength,
                    "metadata": {
                        "beat_index": index,
                        "bar_index": index // 4,
                        "tempo": tempo_value,
                        "meter": 4,
                        "beat_strength": beat_strength,
                        "source": "librosa.beat_track",
                    },
                }
            )
        payload = {
            "version": 1,
            "kind": "beat-grid",
            "duration": _duration_seconds(y, sr),
            "tempo": tempo_value,
            "beat_times": [round(float(value), 6) for value in beat_times[:max_markers]],
            "settings": {"hop_length": hop_length, "max_markers": max_markers},
        }
        return MusicAnalysisResult(kind="beat-grid", payload=payload, markers=markers)

    def analyze_energy(self, audio_path: str | Path, settings: dict[str, Any] | None = None) -> MusicAnalysisResult:
        settings = dict(settings or {})
        hop_length = _positive_int(settings.get("hop_length", DEFAULT_HOP_LENGTH), "hop_length")
        max_frames = _positive_int(settings.get("max_frames", DEFAULT_MAX_FRAMES), "max_frames")
        max_markers = _positive_int(settings.get("max_markers", DEFAULT_MAX_MARKERS), "max_markers")
        y, sr = _load_audio(audio_path)
        rms = librosa.feature.rms(y=y, hop_length=hop_length)[0]
        onset_env = librosa.onset.onset_strength(y=y, sr=sr, hop_length=hop_length)
        times = librosa.frames_to_time(np.arange(len(rms)), sr=sr, hop_length=hop_length)
        intensity = _normalize(rms + _resize(onset_env, len(rms)))
        frames = _decimated_frames(times, intensity, max_frames, "intensity")
        markers = _energy_markers(times, intensity, max_markers)
        payload = {
            "version": 1,
            "kind": "energy",
            "duration": _duration_seconds(y, sr),
            "frames": frames,
            "settings": {"hop_length": hop_length, "max_frames": max_frames, "max_markers": max_markers},
        }
        return MusicAnalysisResult(kind="energy", payload=payload, markers=markers, frames=frames)

    def analyze_harmony(self, audio_path: str | Path, settings: dict[str, Any] | None = None) -> MusicAnalysisResult:
        settings = dict(settings or {})
        hop_length = _positive_int(settings.get("hop_length", DEFAULT_HOP_LENGTH), "hop_length")
        max_frames = _positive_int(settings.get("max_frames", DEFAULT_MAX_FRAMES), "max_frames")
        max_markers = _positive_int(settings.get("max_markers", DEFAULT_MAX_MARKERS), "max_markers")
        y, sr = _load_audio(audio_path)
        try:
            chroma = librosa.feature.chroma_cqt(y=y, sr=sr, hop_length=hop_length)
        except ParameterError:
            chroma = librosa.feature.chroma_stft(y=y, sr=sr, hop_length=hop_length)
        times = librosa.frames_to_time(np.arange(chroma.shape[1]), sr=sr, hop_length=hop_length)
        frames = _chroma_frames(times, chroma, max_frames)
        markers = _harmonic_change_markers(frames, max_markers)
        payload = {
            "version": 1,
            "kind": "harmonic-color",
            "duration": _duration_seconds(y, sr),
            "frames": frames,
            "settings": {"hop_length": hop_length, "max_frames": max_frames, "max_markers": max_markers},
        }
        return MusicAnalysisResult(kind="harmonic-color", payload=payload, markers=markers, frames=frames)


def _load_audio(audio_path: str | Path):
    with warnings.catch_warnings():
        warnings.filterwarnings("ignore", message=".*standard-'?aifc'?.*", category=DeprecationWarning)
        warnings.filterwarnings("ignore", message=".*standard-'?sunau'?.*", category=DeprecationWarning)
        warnings.filterwarnings("ignore", message="n_fft=.*", category=UserWarning)
        return librosa.load(str(audio_path), sr=None, mono=True)


def _positive_int(value: Any, name: str) -> int:
    try:
        result = int(value)
    except (TypeError, ValueError, OverflowError) as exc:
        raise ValueError(f"{name} must be a positive integer") from exc
    if result <= 0:
        raise ValueError(f"{name} must be a positive integer")
    return result


def _first_float(value: Any) -> float:
    values = np.asarray(value).reshape(-1)
    if values.size == 0:
        return 0.0
    result = float(values[0])
    return result if math.isfinite(result) else 0.0


def _duration_seconds(y: np.ndarray, sr: int) -> float:
    return 0.0 if sr <= 0 else float(len(y) / sr)


def _frame_value(values: np.ndarray, index: int) -> float:
    if len(values) == 0:
        return 0.0
    return float(max(0.0, min(1.0, _normalize(values)[max(0, min(index, len(values) - 1))])))


def _resize(values: np.ndarray, size: int) -> np.ndarray:
    if len(values) == size:
        return values
    if size <= 0:
        return np.asarray([])
    if len(values) == 0:
        return np.zeros(size)
    return np.interp(np.linspace(0, len(values) - 1, size), np.arange(len(values)), values)


def _normalize(values: np.ndarray) -> np.ndarray:
    values = np.asarray(values, dtype=float)
    if values.size == 0:
        return values
    min_value = float(np.nanmin(values))
    max_value = float(np.nanmax(values))
    if not math.isfinite(min_value) or not math.isfinite(max_value) or max_value <= min_value:
        return np.zeros_like(values, dtype=float)
    return np.clip((values - min_value) / (max_value - min_value), 0.0, 1.0)


def _decimated_frames(times: np.ndarray, values: np.ndarray, max_frames: int, value_key: str) -> list[dict[str, float]]:
    if len(values) == 0:
        return []
    stride = max(1, math.ceil(len(values) / max_frames))
    return [
        {"time": round(float(times[index]), 6), value_key: round(float(values[index]), 6)}
        for index in range(0, len(values), stride)
    ][:max_frames]


def _energy_markers(times: np.ndarray, intensity: np.ndarray, max_markers: int) -> list[dict[str, Any]]:
    if len(intensity) == 0:
        return []
    threshold = max(0.65, float(np.mean(intensity) + np.std(intensity)))
    markers = []
    for index in range(1, len(intensity) - 1):
        value = float(intensity[index])
        if value >= threshold and value >= intensity[index - 1] and value >= intensity[index + 1]:
            markers.append(
                {
                    "timestamp": round(float(times[index]), 6),
                    "label": "Energy Peak",
                    "category": "energy_peak",
                    "confidence": round(value, 6),
                    "metadata": {"intensity": round(value, 6), "source": "rms_onset_intensity"},
                }
            )
    return markers[:max_markers]


def _chroma_frames(times: np.ndarray, chroma: np.ndarray, max_frames: int) -> list[dict[str, Any]]:
    if chroma.size == 0:
        return []
    stride = max(1, math.ceil(chroma.shape[1] / max_frames))
    frames = []
    for frame_index in range(0, chroma.shape[1], stride):
        vector = np.asarray(chroma[:, frame_index], dtype=float)
        normalized = _normalize(vector)
        dominant = int(np.argmax(normalized)) if normalized.size else 0
        frames.append(
            {
                "time": round(float(times[frame_index]), 6),
                "chroma": [round(float(value), 6) for value in normalized[:12]],
                "color": _color_for_pitch_class(dominant),
                "dominant_pitch_class": dominant,
            }
        )
    return frames[:max_frames]


def _color_for_pitch_class(pitch_class: int) -> str:
    hue = int((pitch_class % 12) * 30)
    return f"hsl({hue}, 72%, 58%)"


def _harmonic_change_markers(frames: list[dict[str, Any]], max_markers: int) -> list[dict[str, Any]]:
    markers = []
    previous = None
    for frame in frames:
        current = frame.get("dominant_pitch_class")
        if previous is not None and current != previous:
            markers.append(
                {
                    "timestamp": float(frame["time"]),
                    "label": "Harmonic Change",
                    "category": "harmonic_change",
                    "confidence": 0.75,
                    "metadata": {"previous_pitch_class": previous, "pitch_class": current},
                }
            )
        previous = current
    return markers[:max_markers]
