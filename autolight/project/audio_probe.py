from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

import soundfile


@dataclass(frozen=True, slots=True)
class AudioMetadata:
    duration: float
    sample_rate: int
    channels: int


def probe_audio_file(path: str | Path) -> AudioMetadata:
    audio_path = Path(path)
    if audio_path.exists() and not audio_path.is_file():
        raise IsADirectoryError(f"audio asset path is not a file: {audio_path}")
    if not audio_path.is_file():
        raise FileNotFoundError(str(audio_path))

    try:
        info = soundfile.info(str(audio_path))
    except Exception:
        return _probe_with_librosa(audio_path)

    duration = 0.0 if info.samplerate == 0 else info.frames / info.samplerate
    return AudioMetadata(
        duration=float(duration),
        sample_rate=int(info.samplerate),
        channels=int(info.channels),
    )


def _probe_with_librosa(audio_path: Path) -> AudioMetadata:
    import librosa

    audio, sample_rate = librosa.load(str(audio_path), sr=None, mono=False)
    shape = getattr(audio, "shape", ())
    channels = 1
    frames = len(audio)
    if len(shape) >= 2:
        channels = int(shape[0])
        frames = int(shape[-1])

    duration = 0.0 if sample_rate == 0 else frames / sample_rate
    return AudioMetadata(
        duration=float(duration),
        sample_rate=int(sample_rate),
        channels=channels,
    )
