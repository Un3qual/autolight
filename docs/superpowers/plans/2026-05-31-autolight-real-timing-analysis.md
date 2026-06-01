# Autolight Real Timing Analysis Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic real-audio timing transforms for onsets and beats using `librosa`, while keeping the existing fixed-interval transform for fast tests and demos.

**Architecture:** Put audio-analysis code in a focused `autolight.analysis.timing` module. Built-in transform wrappers pass an `audio_path` parameter and produce marker dictionaries that the existing job queue already persists.

**Tech Stack:** Python 3.14, `librosa`, `numpy`, `soundfile`, `unittest`, existing transform registry and job queue.

---

## File Structure

- Create `autolight/analysis/timing.py`: onset and beat marker extraction helpers.
- Modify `autolight/analysis/builtin.py`: register `timing.onsets` and `timing.beats` transforms.
- Create `tests/test_timing_analysis.py`: synthetic click-track coverage and transform registration tests.
- Modify `README.md`: list timing transforms as available MVP transforms.

## Task 1: Timing Analysis Helpers

**Files:**
- Create: `autolight/analysis/timing.py`
- Create: `tests/test_timing_analysis.py`

- [x] **Step 1: Write failing timing helper tests**

Create `tests/test_timing_analysis.py`:

```python
import tempfile
import unittest
from pathlib import Path

import numpy as np
import soundfile

from autolight.analysis.timing import detect_beat_markers, detect_onset_markers


def write_click_track(path: Path, sample_rate: int = 22050) -> None:
    audio = np.zeros(sample_rate * 2, dtype=np.float32)
    audio[0] = 1.0
    audio[sample_rate // 2] = 1.0
    audio[sample_rate] = 1.0
    audio[(sample_rate * 3) // 2] = 1.0
    soundfile.write(str(path), audio, sample_rate)


class TimingAnalysisTest(unittest.TestCase):
    def test_detect_onset_markers_returns_timestamped_markers(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "clicks.wav"
            write_click_track(audio_path)

            markers = detect_onset_markers(audio_path)

        self.assertGreaterEqual(len(markers), 2)
        self.assertTrue(all(marker["category"] == "onset" for marker in markers))
        self.assertTrue(all(marker["timestamp"] >= 0 for marker in markers))

    def test_detect_beat_markers_returns_timing_markers(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "clicks.wav"
            write_click_track(audio_path)

            markers = detect_beat_markers(audio_path)

        self.assertGreaterEqual(len(markers), 1)
        self.assertTrue(all(marker["label"] == "Beat" for marker in markers))


if __name__ == "__main__":
    unittest.main()
```

- [x] **Step 2: Run timing tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_timing_analysis -v
```

Expected: FAIL with `ModuleNotFoundError: No module named 'autolight.analysis.timing'`.

- [x] **Step 3: Implement timing helpers**

Create `autolight/analysis/timing.py`:

```python
from __future__ import annotations

from pathlib import Path

import librosa


def detect_onset_markers(audio_path: str | Path) -> list[dict]:
    y, sr = librosa.load(str(audio_path), sr=None, mono=True)
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
    y, sr = librosa.load(str(audio_path), sr=None, mono=True)
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
```

- [x] **Step 4: Run timing helper tests**

Run:

```bash
uv run python -m unittest tests.test_timing_analysis -v
```

Expected: PASS for the helper tests.

- [x] **Step 5: Commit timing helpers**

Run:

```bash
git add autolight/analysis/timing.py tests/test_timing_analysis.py
git commit -m "Add timing analysis helpers"
```

Expected: commit succeeds.

## Task 2: Built-In Timing Transforms

**Files:**
- Modify: `autolight/analysis/builtin.py`
- Modify: `tests/test_timing_analysis.py`

- [x] **Step 1: Add failing transform tests**

Add these imports:

```python
from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformContext, TransformRegistry
```

Add this test:

```python
    def test_timing_transforms_are_registered_and_return_markers(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "clicks.wav"
            write_click_track(audio_path)
            context = TransformContext(
                artifact_dir=Path(tmp) / "artifacts",
                cancel_requested=lambda: False,
                progress=lambda value: None,
            )

            onset_result = registry.get("timing.onsets", version="1").run(context, {"audio_path": str(audio_path)})
            beat_result = registry.get("timing.beats", version="1").run(context, {"audio_path": str(audio_path)})

        self.assertGreaterEqual(len(onset_result.markers), 2)
        self.assertGreaterEqual(len(beat_result.markers), 1)
```

- [x] **Step 2: Run transform test and verify failure**

Run:

```bash
uv run python -m unittest tests.test_timing_analysis.TimingAnalysisTest.test_timing_transforms_are_registered_and_return_markers -v
```

Expected: FAIL because timing transforms are not registered.

- [x] **Step 3: Register timing transforms**

Add imports:

```python
from autolight.analysis.timing import detect_beat_markers, detect_onset_markers
```

Register transforms:

```python
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
```

Add functions:

```python
def _timing_onsets(context: TransformContext, params: dict) -> TransformResult:
    context.progress(0.1)
    markers = detect_onset_markers(Path(str(params["audio_path"])))
    if context.cancel_requested():
        raise TransformCancelled("cancelled")
    context.progress(1.0)
    return TransformResult(markers=markers)


def _timing_beats(context: TransformContext, params: dict) -> TransformResult:
    context.progress(0.1)
    markers = detect_beat_markers(Path(str(params["audio_path"])))
    if context.cancel_requested():
        raise TransformCancelled("cancelled")
    context.progress(1.0)
    return TransformResult(markers=markers)
```

- [x] **Step 4: Run timing analysis tests**

Run:

```bash
uv run python -m unittest tests.test_timing_analysis tests.test_analysis -v
```

Expected: PASS.

- [x] **Step 5: Commit timing transforms**

Run:

```bash
git add autolight/analysis/builtin.py tests/test_timing_analysis.py
git commit -m "Register timing analysis transforms"
```

Expected: commit succeeds.

## Final Verification

- [x] **Step 1: Run full tests**

Run:

```bash
uv run python -m unittest discover -s tests -v
```

Expected: all tests pass.

- [x] **Step 2: Run smoke and diff check**

Run:

```bash
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
git diff --check
```

Expected: smoke exits 0 and diff check has no output.
