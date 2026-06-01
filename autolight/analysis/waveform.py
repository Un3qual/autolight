from __future__ import annotations

import json
import math
from pathlib import Path

import numpy as np
import soundfile

WAVEFORM_READ_BLOCK_FRAMES = 65_536


def build_waveform_summary(audio_path: str | Path, output_path: str | Path, buckets: int = 512) -> None:
    if buckets <= 0:
        raise ValueError("buckets must be greater than zero")

    with soundfile.SoundFile(str(audio_path)) as audio:
        sample_rate = int(audio.samplerate)
        frame_count = int(audio.frames)
        samples = _summarize_audio_buckets(audio, frame_count, buckets)

    payload = {
        "version": 1,
        "sample_rate": sample_rate,
        "duration": 0.0 if sample_rate == 0 else float(frame_count / sample_rate),
        "samples": samples,
    }
    Path(output_path).write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")


def _summarize_audio_buckets(audio, frame_count: int, buckets: int) -> list[dict[str, float]]:
    if frame_count <= 0:
        return []

    bucket_count = min(buckets, frame_count)
    samples = []
    for bucket_index in range(bucket_count):
        start = math.floor(bucket_index * frame_count / bucket_count)
        stop = math.floor((bucket_index + 1) * frame_count / bucket_count)
        samples.append(_summarize_frame_range(audio, start, stop))
    return samples


def _summarize_frame_range(audio, start: int, stop: int) -> dict[str, float]:
    audio.seek(start)
    remaining = max(0, stop - start)
    peak = 0.0
    square_total = 0.0
    frame_total = 0

    while remaining > 0:
        data = audio.read(
            min(remaining, WAVEFORM_READ_BLOCK_FRAMES),
            always_2d=True,
            dtype="float32",
        )
        frames_read = data.shape[0]
        if frames_read == 0:
            break
        mono = np.mean(data, axis=1)
        if mono.size:
            peak = max(peak, float(np.max(np.abs(mono))))
            square_total += float(np.sum(np.square(mono), dtype=np.float64))
            frame_total += int(mono.size)
        remaining -= frames_read

    if frame_total == 0:
        return {"peak": 0.0, "rms": 0.0}
    return {"peak": peak, "rms": float(math.sqrt(square_total / frame_total))}
