from pathlib import Path

from autolight.demo_audio import write_silent_wav


def write_wav(path: Path, *, sample_rate: int = 8000, channels: int = 1, frames: int = 8000) -> None:
    write_silent_wav(path, sample_rate=sample_rate, channels=channels, frames=frames)
