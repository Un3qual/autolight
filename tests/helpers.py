import wave
from pathlib import Path


def write_wav(path: Path, *, sample_rate: int = 8000, channels: int = 1, frames: int = 8000) -> None:
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(channels)
        handle.setsampwidth(2)
        handle.setframerate(sample_rate)
        handle.writeframes(b"\0\0" * frames * channels)
