from __future__ import annotations

import warnings
from pathlib import Path

import librosa


def detect_onset_markers(audio_path: str | Path) -> list[dict]:
    y, sr = _load_audio(audio_path)
    frames = librosa.onset.onset_detect(y=y, sr=sr, units="frames", backtrack=False)
    times = librosa.frames_to_time(frames, sr=sr)
    return [
        {
            "timestamp": round(float(timestamp), 6),
            "label": "Onset",
            "category": "onset",
            "confidence": 1.0,
            "metadata": {"source": "librosa.onset_detect"},
        }
        for timestamp in times
    ]


def detect_beat_markers(audio_path: str | Path) -> list[dict]:
    y, sr = _load_audio(audio_path)
    tempo, frames = librosa.beat.beat_track(y=y, sr=sr, units="frames")
    times = librosa.frames_to_time(frames, sr=sr)
    tempo_value = float(tempo[0] if hasattr(tempo, "__len__") else tempo)
    return [
        {
            "timestamp": round(float(timestamp), 6),
            "label": "Beat",
            "category": "beat",
            "confidence": 1.0,
            "metadata": {"tempo": tempo_value, "source": "librosa.beat_track"},
        }
        for timestamp in times
    ]


def _load_audio(audio_path: str | Path):
    with warnings.catch_warnings():
        warnings.filterwarnings("ignore", message="aifc was removed.*", category=DeprecationWarning)
        warnings.filterwarnings("ignore", message="sunau was removed.*", category=DeprecationWarning)
        return librosa.load(str(audio_path), sr=None, mono=True)
