# Autolight Waveform Summary Cache Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Generate cache-backed waveform preview artifacts for imported source tracks and expose compact waveform samples to QML.

**Architecture:** Implement waveform summary as a built-in transform that writes a JSON artifact through the existing job/cache pipeline. Add a small loader that reads validated waveform artifacts into a timeline role, keeping QML free of file parsing.

**Tech Stack:** Python 3.14, `numpy`, `soundfile`, PySide6/QML, `unittest`, existing `CacheStore`, `LocalJobQueue`, and `TimelineTrackModel`.

---

## File Structure

- Create `autolight/analysis/waveform.py`: deterministic peak/RMS summary generation from local audio files.
- Modify `autolight/analysis/builtin.py`: register `waveform.summary` transform.
- Modify `autolight/app_controller.py`: load completed waveform cache artifacts into track provenance when jobs finish.
- Modify `autolight/timeline/model.py`: add `waveformSamples` role backed by the controller-loaded cache artifact samples.
- Create `tests/test_waveform_summary.py`: waveform transform and model role coverage.
- Modify `UI/Main.qml`: draw a simple waveform strip for tracks with summary samples.

## Task 1: Waveform Summary Generator

**Files:**
- Create: `autolight/analysis/waveform.py`
- Create: `tests/test_waveform_summary.py`

- [x] **Step 1: Write failing waveform generator tests**

Create `tests/test_waveform_summary.py`:

```python
import json
import tempfile
import unittest
import wave
from pathlib import Path

from autolight.analysis.waveform import build_waveform_summary


def write_wav(path: Path) -> None:
    samples = [0, 1000, -1000, 2000, -2000, 0, 500, -500]
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(8)
        handle.writeframes(b"".join(sample.to_bytes(2, "little", signed=True) for sample in samples))


class WaveformSummaryTest(unittest.TestCase):
    def test_build_waveform_summary_returns_normalized_buckets(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            output_path = Path(tmp) / "waveform.json"
            write_wav(audio_path)

            build_waveform_summary(audio_path, output_path, buckets=4)
            payload = json.loads(output_path.read_text(encoding="utf-8"))

        self.assertEqual(payload["version"], 1)
        self.assertEqual(payload["sample_rate"], 8)
        self.assertEqual(len(payload["samples"]), 4)
        self.assertTrue(all(0.0 <= item["peak"] <= 1.0 for item in payload["samples"]))
        self.assertTrue(all(0.0 <= item["rms"] <= 1.0 for item in payload["samples"]))


if __name__ == "__main__":
    unittest.main()
```

- [x] **Step 2: Run waveform generator tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_waveform_summary -v
```

Expected: FAIL with `ModuleNotFoundError: No module named 'autolight.analysis.waveform'`.

- [x] **Step 3: Implement waveform generator**

Create `autolight/analysis/waveform.py`:

```python
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
```

- [x] **Step 4: Run waveform generator tests**

Run:

```bash
uv run python -m unittest tests.test_waveform_summary -v
```

Expected: PASS with 1 test.

- [x] **Step 5: Commit waveform generator**

Run:

```bash
git add autolight/analysis/waveform.py tests/test_waveform_summary.py
git commit -m "Add waveform summary generator"
```

Expected: commit succeeds.

## Task 2: Register Waveform Transform

**Files:**
- Modify: `autolight/analysis/builtin.py`
- Modify: `tests/test_waveform_summary.py`

- [x] **Step 1: Add failing transform registration test**

Add these imports to `tests/test_waveform_summary.py`:

```python
from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformContext, TransformRegistry
```

Add this test:

```python
    def test_waveform_summary_transform_writes_artifact(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            artifact_dir = root / "artifacts"
            write_wav(audio_path)
            transform = registry.get("waveform.summary", version="1")
            result = transform.run(
                TransformContext(
                    artifact_dir=artifact_dir,
                    cancel_requested=lambda: False,
                    progress=lambda value: None,
                ),
                {"audio_path": str(audio_path), "buckets": 4},
            )

        self.assertEqual(set(result.artifacts), {"waveform"})
        self.assertTrue(Path(result.artifacts["waveform"]).name.endswith(".json"))
        self.assertEqual(result.metadata["bucket_count"], 4)
```

- [x] **Step 2: Run transform test and verify failure**

Run:

```bash
uv run python -m unittest tests.test_waveform_summary.WaveformSummaryTest.test_waveform_summary_transform_writes_artifact -v
```

Expected: FAIL because `waveform.summary` is not registered.

- [x] **Step 3: Register `waveform.summary` transform**

Add this import to `autolight/analysis/builtin.py`:

```python
from autolight.analysis.waveform import build_waveform_summary
```

Register this transform in `register_builtin_transforms`:

```python
    registry.register(
        TransformSpec(
            id="waveform.summary",
            version="1",
            name="Waveform Summary",
            input_schema="audio.v1",
            output_schema="artifact.waveform.v1",
            estimated_cost="medium",
            run=_waveform_summary,
        )
    )
```

Add this function:

```python
def _waveform_summary(context: TransformContext, params: dict) -> TransformResult:
    audio_path = Path(str(params["audio_path"]))
    buckets = int(params.get("buckets", 512))
    context.artifact_dir.mkdir(parents=True, exist_ok=True)
    context.progress(0.1)
    output_path = Path(context.artifact_dir) / "waveform.json"
    build_waveform_summary(audio_path, output_path, buckets=buckets)
    if context.cancel_requested():
        raise TransformCancelled("cancelled")
    context.progress(1.0)
    return TransformResult(
        artifacts={"waveform": str(output_path)},
        metadata={"bucket_count": buckets},
    )
```

- [x] **Step 4: Run waveform tests**

Run:

```bash
uv run python -m unittest tests.test_waveform_summary -v
```

Expected: PASS.

- [x] **Step 5: Commit waveform transform**

Run:

```bash
git add autolight/analysis/builtin.py tests/test_waveform_summary.py
git commit -m "Register waveform summary transform"
```

Expected: commit succeeds.

## Task 3: Timeline Waveform Role And QML Strip

**Files:**
- Modify: `autolight/app_controller.py`
- Modify: `autolight/timeline/model.py`
- Modify: `UI/Main.qml`
- Modify: `tests/test_waveform_summary.py`

- [x] **Step 1: Add failing model and QML tests**

Add this test to `WaveformSummaryTest`:

```python
    def test_controller_loads_waveform_samples_after_job_completion(self):
        from autolight.app_controller import AppController
        from autolight.project.store import add_generated_track, import_audio_asset

        controller = AppController()
        self.addCleanup(controller.cleanup)

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            source = import_audio_asset(controller._project, audio_path)
            track = add_generated_track(
                controller._project,
                parent_track_id=source.id,
                name="Waveform",
                transform_id="waveform.summary",
                transform_params={"audio_path": str(audio_path), "buckets": 4},
                transform_version="1",
                output_schema="artifact.waveform.v1",
                dependency_hash="waveform-test",
            )
            controller.trackModel.set_project(controller._project)

            job_id = controller.run_track(track.id)
            controller._job_queue.wait(job_id, timeout=5)

        model = controller.trackModel
        waveform_role = model.role_for_name("waveformSamples")
        row = next(index for index, item in enumerate(controller._project.tracks) if item.id == track.id)
        samples = model.data(model.index(row, 0), waveform_role)

        self.assertEqual(len(samples), 4)
        self.assertIn("peak", samples[0])

    def test_qml_mentions_waveform_samples_role(self):
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")
        self.assertIn("waveformSamples", qml)
        self.assertIn("modelData.peak", qml)
```

- [x] **Step 2: Run QML test and verify failure**

Run:

```bash
uv run python -m unittest tests.test_waveform_summary.WaveformSummaryTest.test_controller_loads_waveform_samples_after_job_completion tests.test_waveform_summary.WaveformSummaryTest.test_qml_mentions_waveform_samples_role -v
```

Expected: FAIL because waveform jobs do not yet load cached JSON samples into the track model and QML does not yet reference `waveformSamples`.

- [x] **Step 3: Load waveform artifacts, add waveform role, and add QML repeater**

Add imports to `autolight/app_controller.py`:

```python
import json

from autolight.project.store import find_track
```

Change the `LocalJobQueue` setup so controller code sees completed track changes before the model refresh signal. The job-progress-controls plan owns progress notification changes inside `LocalJobQueue._run`; this waveform plan only requires those notifications to be present if the job-progress plan has not already landed:

```python
            on_track_changed=self._handle_track_changed,
```

Add these helpers to `AppController`:

```python
    def _handle_track_changed(self, track_id: str) -> None:
        self._load_waveform_samples(track_id)
        self._track_model.trackChangedRequested.emit(track_id)

    def _load_waveform_samples(self, track_id: str) -> None:
        track = find_track(self._project, track_id)
        if track is None or track.transform_id != "waveform.summary":
            return
        entries_by_id = {entry.id: entry for entry in self._project.cache_entries}
        for cache_ref in track.cache_refs:
            entry = entries_by_id.get(cache_ref)
            if entry is None or entry.artifact_kind != "waveform":
                continue
            artifact_path = self._job_queue.cache_store.artifact_path(entry)
            try:
                payload = json.loads(artifact_path.read_text(encoding="utf-8"))
            except (OSError, ValueError):
                return
            samples = payload.get("samples", [])
            if isinstance(samples, list):
                track.provenance["waveform_samples"] = samples
            return
```

Add a timeline model role:

```python
        Qt.ItemDataRole.UserRole + 11: b"waveformSamples",
```

Return the waveform sample list that the controller loaded from the completed cache artifact:

```python
        if role == self.role_for_name("waveformSamples"):
            return track.provenance.get("waveform_samples", [])
```

Add this `Repeater` before marker rendering in the timeline lane `Rectangle`:

```qml
                    Repeater {
                        model: waveformSamples
                        Rectangle {
                            width: 2
                            height: Math.max(2, modelData.peak * (parent.height - 18))
                            x: index * 3
                            y: (parent.height - height) / 2
                            color: "#60a5fa"
                        }
                    }
```

- [x] **Step 4: Run waveform tests and smoke**

Run:

```bash
uv run python -m unittest tests.test_waveform_summary tests.test_timeline_model -v
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: tests pass and smoke exits 0.

- [x] **Step 5: Commit waveform timeline display**

Run:

```bash
git add autolight/app_controller.py autolight/timeline/model.py UI/Main.qml tests/test_waveform_summary.py
git commit -m "Expose waveform summaries in timeline"
```

Expected: commit succeeds.

## Final Verification

- [x] **Step 1: Run full tests**

Run:

```bash
uv run python -m unittest discover -s tests -v
```

Expected: all tests pass.

- [x] **Step 2: Run smoke and diff checks**

Run:

```bash
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
git diff --check
```

Expected: smoke exits 0 and diff check has no output.
