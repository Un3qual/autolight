from __future__ import annotations

import json
from pathlib import Path

import numpy as np
import soundfile


def build_waveform_summary(audio_path: str | Path, output_path: str | Path, buckets: int = 512) -> None:
    if buckets <= 0:
        raise ValueError("buckets must be greater than zero")

    data, sample_rate = soundfile.read(str(audio_path), always_2d=True, dtype="float32")
    mono = np.mean(data, axis=1)
    if mono.size == 0:
        samples = []
    else:
        chunks = np.array_split(mono, min(buckets, mono.size))
        samples = [
            {
                "peak": float(np.max(np.abs(chunk))),
                "rms": float(np.sqrt(np.mean(np.square(chunk)))),
            }
            for chunk in chunks
        ]

    payload = {
        "version": 1,
        "sample_rate": int(sample_rate),
        "duration": 0.0 if sample_rate == 0 else float(mono.size / sample_rate),
        "samples": samples,
    }
    Path(output_path).write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")
