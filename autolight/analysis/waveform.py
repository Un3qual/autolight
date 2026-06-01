from __future__ import annotations

import json
import math
from collections.abc import Callable, Iterable
from pathlib import Path

import numpy as np
import soundfile

from autolight.analysis.registry import TransformCancelled

WAVEFORM_READ_BLOCK_FRAMES = 65_536


def build_waveform_summary(
    audio_path: str | Path,
    output_path: str | Path,
    buckets: int = 512,
    cancel_requested: Callable[[], bool] | None = None,
) -> None:
    if buckets <= 0:
        raise ValueError("buckets must be greater than zero")

    _raise_if_cancelled(cancel_requested)
    try:
        sample_rate, frame_count, samples = _build_soundfile_summary(
            audio_path,
            buckets,
            cancel_requested,
        )
    except (soundfile.SoundFileError, RuntimeError):
        sample_rate, frame_count, samples = _build_audioread_summary(
            audio_path,
            buckets,
            cancel_requested,
        )

    payload = {
        "version": 1,
        "sample_rate": sample_rate,
        "duration": 0.0 if sample_rate == 0 else float(frame_count / sample_rate),
        "samples": samples,
    }
    Path(output_path).write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")


def _build_soundfile_summary(
    audio_path: str | Path,
    buckets: int,
    cancel_requested: Callable[[], bool] | None,
) -> tuple[int, int, list[dict[str, float]]]:
    with soundfile.SoundFile(str(audio_path)) as audio:
        sample_rate = int(audio.samplerate)
        frame_count = int(audio.frames)
        samples = _summarize_audio_buckets(audio, frame_count, buckets, cancel_requested)
    return sample_rate, frame_count, samples


def _build_audioread_summary(
    audio_path: str | Path,
    buckets: int,
    cancel_requested: Callable[[], bool] | None,
) -> tuple[int, int, list[dict[str, float]]]:
    try:
        import audioread
    except ImportError as exc:
        raise RuntimeError(
            "audioread is required to summarize this unsupported audio container"
        ) from exc

    with audioread.audio_open(str(audio_path)) as reader:
        sample_rate = int(reader.samplerate)
        channel_count = int(reader.channels)
        frame_count = max(0, int(round(float(reader.duration) * sample_rate)))
        samples = _summarize_mono_chunks(
            _audioread_mono_chunks(reader, channel_count),
            frame_count,
            buckets,
            cancel_requested,
        )
    return sample_rate, frame_count, samples


def _summarize_audio_buckets(
    audio,
    frame_count: int,
    buckets: int,
    cancel_requested: Callable[[], bool] | None,
) -> list[dict[str, float]]:
    if frame_count <= 0:
        return []

    bucket_count = min(buckets, frame_count)
    samples = []
    for bucket_index in range(bucket_count):
        _raise_if_cancelled(cancel_requested)
        start = math.floor(bucket_index * frame_count / bucket_count)
        stop = math.floor((bucket_index + 1) * frame_count / bucket_count)
        samples.append(_summarize_frame_range(audio, start, stop, cancel_requested))
    return samples


def _summarize_frame_range(
    audio,
    start: int,
    stop: int,
    cancel_requested: Callable[[], bool] | None,
) -> dict[str, float]:
    audio.seek(start)
    remaining = max(0, stop - start)
    peak = 0.0
    square_total = 0.0
    frame_total = 0

    while remaining > 0:
        _raise_if_cancelled(cancel_requested)
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


def _audioread_mono_chunks(reader, channel_count: int) -> Iterable[np.ndarray]:
    channels = max(1, channel_count)
    for block in reader:
        pcm = np.frombuffer(block, dtype="<i2")
        frame_count = int(pcm.size // channels)
        if frame_count == 0:
            continue
        frames = pcm[: frame_count * channels].reshape(frame_count, channels)
        yield np.mean(frames.astype(np.float32) / 32768.0, axis=1)


def _summarize_mono_chunks(
    chunks: Iterable[np.ndarray],
    frame_count: int,
    buckets: int,
    cancel_requested: Callable[[], bool] | None,
) -> list[dict[str, float]]:
    if frame_count <= 0:
        return []

    bucket_count = min(buckets, frame_count)
    accumulators = [_BucketAccumulator() for _ in range(bucket_count)]
    frame_cursor = 0
    bucket_index = 0

    for mono in chunks:
        _raise_if_cancelled(cancel_requested)
        if mono.size == 0 or bucket_index >= bucket_count:
            continue
        chunk_start = frame_cursor
        chunk_stop = min(frame_cursor + int(mono.size), frame_count)
        position = chunk_start

        while position < chunk_stop and bucket_index < bucket_count:
            _raise_if_cancelled(cancel_requested)
            bucket_stop = math.floor((bucket_index + 1) * frame_count / bucket_count)
            if position >= bucket_stop:
                bucket_index += 1
                continue
            segment_stop = min(chunk_stop, bucket_stop)
            segment = mono[position - chunk_start : segment_stop - chunk_start]
            accumulators[bucket_index].add(segment)
            position = segment_stop
        frame_cursor += int(mono.size)

    return [accumulator.summary() for accumulator in accumulators]


class _BucketAccumulator:
    def __init__(self):
        self.peak = 0.0
        self.square_total = 0.0
        self.frame_total = 0

    def add(self, mono: np.ndarray) -> None:
        if mono.size == 0:
            return
        self.peak = max(self.peak, float(np.max(np.abs(mono))))
        self.square_total += float(np.sum(np.square(mono), dtype=np.float64))
        self.frame_total += int(mono.size)

    def summary(self) -> dict[str, float]:
        if self.frame_total == 0:
            return {"peak": 0.0, "rms": 0.0}
        return {"peak": self.peak, "rms": float(math.sqrt(self.square_total / self.frame_total))}


def _raise_if_cancelled(cancel_requested: Callable[[], bool] | None) -> None:
    if cancel_requested is not None and cancel_requested():
        raise TransformCancelled("cancelled")
