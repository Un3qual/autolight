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
    except (soundfile.SoundFileError, RuntimeError):
        return _probe_with_audioread(audio_path)

    duration = 0.0 if info.samplerate == 0 else info.frames / info.samplerate
    return AudioMetadata(
        duration=float(duration),
        sample_rate=int(info.samplerate),
        channels=int(info.channels),
    )


def _probe_with_audioread(audio_path: Path) -> AudioMetadata:
    try:
        import audioread
    except ImportError as exc:
        raise RuntimeError(
            "audioread is required to probe this unsupported audio container"
        ) from exc

    with audioread.audio_open(str(audio_path)) as reader:
        return AudioMetadata(
            duration=float(reader.duration),
            sample_rate=int(reader.samplerate),
            channels=int(reader.channels),
        )
