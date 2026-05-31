# Autolight Audio Import Metadata Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Populate imported `AudioAsset` records with real duration, sample rate, channel count, online/offline status, and relink behavior.

**Architecture:** Add a small analysis-free audio probing module that uses `librosa.get_duration` and `soundfile.info` through `librosa`'s installed stack, then call it from `import_audio_asset`. Keep project persistence unchanged: the existing `AudioAsset` fields already have the needed shape.

**Tech Stack:** Python 3.14, `librosa`, `soundfile`, `unittest`, existing `ProjectStore` and `AudioAsset` dataclass.

---

## File Structure

- Create `autolight/project/audio_probe.py`: file existence, metadata probing, and offline-state helper.
- Modify `autolight/project/store.py`: call `probe_audio_file` during import and expose `refresh_audio_asset_status`.
- Modify `autolight/project/__init__.py`: export `refresh_audio_asset_status`.
- Create `tests/test_audio_import_metadata.py`: synthetic WAV import, invalid path, offline reload, and relink tests.
- Modify `README.md`: document that imports validate and fingerprint local audio.

## Task 1: Audio Probe Module

**Files:**
- Create: `autolight/project/audio_probe.py`
- Create: `tests/test_audio_import_metadata.py`

- [ ] **Step 1: Write failing probe tests**

Create `tests/test_audio_import_metadata.py`:

```python
import math
import tempfile
import unittest
import wave
from pathlib import Path

from autolight.project.audio_probe import probe_audio_file


def write_wav(path: Path, *, sample_rate: int = 8000, channels: int = 1, frames: int = 8000) -> None:
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(channels)
        handle.setsampwidth(2)
        handle.setframerate(sample_rate)
        handle.writeframes(b"\0\0" * frames * channels)


class AudioImportMetadataTest(unittest.TestCase):
    def test_probe_audio_file_returns_duration_sample_rate_and_channels(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path, sample_rate=8000, channels=2, frames=4000)

            metadata = probe_audio_file(audio_path)

        self.assertTrue(math.isclose(metadata.duration, 0.5, rel_tol=0.01))
        self.assertEqual(metadata.sample_rate, 8000)
        self.assertEqual(metadata.channels, 2)

    def test_probe_audio_file_rejects_directory(self):
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(IsADirectoryError, "not a file"):
                probe_audio_file(Path(tmp))


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run probe tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_audio_import_metadata -v
```

Expected: FAIL with `ModuleNotFoundError: No module named 'autolight.project.audio_probe'`.

- [ ] **Step 3: Implement `audio_probe.py`**

Create `autolight/project/audio_probe.py`:

```python
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

    info = soundfile.info(str(audio_path))
    duration = 0.0 if info.samplerate == 0 else info.frames / info.samplerate
    return AudioMetadata(
        duration=float(duration),
        sample_rate=int(info.samplerate),
        channels=int(info.channels),
    )
```

- [ ] **Step 4: Run probe tests and verify pass**

Run:

```bash
uv run python -m unittest tests.test_audio_import_metadata -v
```

Expected: PASS with 2 tests.

- [ ] **Step 5: Commit audio probe module**

Run:

```bash
git add autolight/project/audio_probe.py tests/test_audio_import_metadata.py
git commit -m "Add audio metadata probe"
```

Expected: commit succeeds.

## Task 2: Import Metadata Into Project Assets

**Files:**
- Modify: `autolight/project/store.py`
- Modify: `tests/test_audio_import_metadata.py`

- [ ] **Step 1: Add failing project import metadata test**

Append this import to `tests/test_audio_import_metadata.py`:

```python
from autolight.project.store import import_audio_asset, new_project
```

Add this test:

```python
    def test_import_audio_asset_populates_real_metadata(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path, sample_rate=11025, channels=1, frames=22050)
            project = new_project("Demo")

            track = import_audio_asset(project, audio_path)

        self.assertEqual(track.name, "song")
        self.assertEqual(len(project.audio_assets), 1)
        asset = project.audio_assets[0]
        self.assertTrue(math.isclose(asset.duration, 2.0, rel_tol=0.01))
        self.assertEqual(asset.sample_rate, 11025)
        self.assertEqual(asset.channels, 1)
        self.assertEqual(asset.import_status, "online")
        self.assertEqual(asset.relink_hint, "")
        self.assertNotEqual(asset.fingerprint, "")
```

- [ ] **Step 2: Run the new import test and verify failure**

Run:

```bash
uv run python -m unittest tests.test_audio_import_metadata.AudioImportMetadataTest.test_import_audio_asset_populates_real_metadata -v
```

Expected: FAIL because `duration`, `sample_rate`, and `channels` are still zero.

- [ ] **Step 3: Use the probe in `import_audio_asset`**

Add this import to `autolight/project/store.py`:

```python
from autolight.project.audio_probe import probe_audio_file
```

Replace the `AudioAsset` construction in `import_audio_asset` with:

```python
    metadata = probe_audio_file(audio_path)
    asset = AudioAsset(
        id=new_id("asset"),
        path=str(audio_path),
        duration=metadata.duration,
        sample_rate=metadata.sample_rate,
        channels=metadata.channels,
        fingerprint=fingerprint_file(audio_path),
    )
```

- [ ] **Step 4: Run audio metadata and project store tests**

Run:

```bash
uv run python -m unittest tests.test_audio_import_metadata tests.test_project_store -v
```

Expected: PASS. Update existing project store tests that write fake `.wav` bytes by importing this file's `write_wav` helper and replacing `audio_path.write_bytes(b"fake audio bytes")` with `write_wav(audio_path)`, so imports use decodable audio.

- [ ] **Step 5: Commit import metadata**

Run:

```bash
git add autolight/project/store.py tests/test_audio_import_metadata.py tests/test_project_store.py
git commit -m "Populate audio asset metadata on import"
```

Expected: commit succeeds.

## Task 3: Offline Status And Relink

**Files:**
- Modify: `autolight/project/store.py`
- Modify: `autolight/project/__init__.py`
- Modify: `tests/test_audio_import_metadata.py`

- [ ] **Step 1: Add failing offline and relink tests**

Add these tests to `AudioImportMetadataTest`:

```python
    def test_refresh_audio_asset_status_marks_missing_files_offline(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            project = new_project("Demo")
            import_audio_asset(project, audio_path)
            audio_path.unlink()

            from autolight.project.store import refresh_audio_asset_status

            changed_ids = refresh_audio_asset_status(project)

        self.assertEqual(changed_ids, [project.audio_assets[0].id])
        self.assertEqual(project.audio_assets[0].import_status, "offline")
        self.assertEqual(project.audio_assets[0].relink_hint, "song.wav")

    def test_refresh_audio_asset_status_relinks_matching_fingerprint(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            old_path = root / "old.wav"
            new_path = root / "new.wav"
            write_wav(old_path)
            payload = old_path.read_bytes()
            project = new_project("Demo")
            import_audio_asset(project, old_path)
            old_path.unlink()
            new_path.write_bytes(payload)

            from autolight.project.store import refresh_audio_asset_status

            changed_ids = refresh_audio_asset_status(project, search_dirs=[root])

        self.assertEqual(changed_ids, [project.audio_assets[0].id])
        self.assertEqual(project.audio_assets[0].path, str(new_path))
        self.assertEqual(project.audio_assets[0].import_status, "online")
        self.assertEqual(project.audio_assets[0].relink_hint, "")
```

- [ ] **Step 2: Run offline tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_audio_import_metadata.AudioImportMetadataTest.test_refresh_audio_asset_status_marks_missing_files_offline tests.test_audio_import_metadata.AudioImportMetadataTest.test_refresh_audio_asset_status_relinks_matching_fingerprint -v
```

Expected: FAIL because `refresh_audio_asset_status` is not defined.

- [ ] **Step 3: Implement `refresh_audio_asset_status`**

Add this function to `autolight/project/store.py` below `import_audio_asset`:

```python
def refresh_audio_asset_status(project: ProjectDocument, search_dirs: list[str | Path] | None = None) -> list[str]:
    changed_asset_ids: list[str] = []
    search_roots = [Path(root) for root in search_dirs or []]

    for asset in project.audio_assets:
        asset_path = Path(asset.path)
        if asset_path.is_file():
            if asset.import_status != "online" or asset.relink_hint:
                asset.import_status = "online"
                asset.relink_hint = ""
                changed_asset_ids.append(asset.id)
            continue

        replacement = _find_relink_candidate(asset.fingerprint, search_roots)
        if replacement is not None:
            asset.path = str(replacement)
            asset.import_status = "online"
            asset.relink_hint = ""
            changed_asset_ids.append(asset.id)
            continue

        hint = asset_path.name
        if asset.import_status != "offline" or asset.relink_hint != hint:
            asset.import_status = "offline"
            asset.relink_hint = hint
            changed_asset_ids.append(asset.id)

    return changed_asset_ids


def _find_relink_candidate(fingerprint: str, search_roots: list[Path]) -> Path | None:
    for root in search_roots:
        if not root.is_dir():
            continue
        for candidate in root.rglob("*"):
            if candidate.is_file() and fingerprint_file(candidate) == fingerprint:
                return candidate
    return None
```

Export it from `autolight/project/__init__.py`:

```python
from autolight.project.store import refresh_audio_asset_status
```

Add `"refresh_audio_asset_status"` to `__all__`.

- [ ] **Step 4: Run audio import tests**

Run:

```bash
uv run python -m unittest tests.test_audio_import_metadata -v
```

Expected: PASS.

- [ ] **Step 5: Commit offline status and relink**

Run:

```bash
git add autolight/project/store.py autolight/project/__init__.py tests/test_audio_import_metadata.py
git commit -m "Track offline audio assets and relink by fingerprint"
```

Expected: commit succeeds.

## Final Verification

- [ ] **Step 1: Run the full unit suite**

Run:

```bash
uv run python -m unittest discover -s tests -v
```

Expected: all tests pass.

- [ ] **Step 2: Run the headless smoke check**

Run:

```bash
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: command exits 0.

- [ ] **Step 3: Check whitespace and status**

Run:

```bash
git diff --check
git status --short --branch
```

Expected: no whitespace errors; status contains only intentional changes or is clean if commits were made.
