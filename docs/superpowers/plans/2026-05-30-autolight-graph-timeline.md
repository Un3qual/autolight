# Autolight Graph Timeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first graph-backed Autolight milestone: project persistence, generated/editable track semantics, local jobs, cache keys, a small transform registry, and a QML timeline shell.

**Architecture:** Python owns project truth, analysis, jobs, cache metadata, and Qt-facing view models. QML renders a timeline projection over the Python model and never runs analysis directly. The first graph implementation validates single-parent generated tracks while storing inputs as a list for future DAG-compatible schema evolution.

**Tech Stack:** Python 3.14, PySide6/QML, `unittest`, dataclasses, JSON project files, thread-based local jobs, SHA-256 cache keys.

---

## File Structure

- Create `autolight/__init__.py`: package marker and exported version.
- Create `autolight/project/__init__.py`: project package exports.
- Create `autolight/project/models.py`: schema enums and dataclasses for projects, assets, tracks, markers, jobs, and cache entries.
- Create `autolight/project/store.py`: JSON save/load, file fingerprinting, graph mutation helpers, stale propagation, and validation.
- Create `autolight/cache/__init__.py`: cache package exports.
- Create `autolight/cache/keys.py`: canonical dependency hashes and cache-key helpers.
- Create `autolight/cache/store.py`: local cache directory helper and cache-entry validation.
- Create `autolight/analysis/__init__.py`: analysis package exports.
- Create `autolight/analysis/registry.py`: transform registry contracts.
- Create `autolight/analysis/builtin.py`: deterministic MVP transforms for marker generation and expensive-transform job behavior.
- Create `autolight/jobs/__init__.py`: jobs package exports.
- Create `autolight/jobs/queue.py`: local background job queue, progress, completion, failure, and cancellation state handling.
- Create `autolight/timeline/__init__.py`: timeline package exports.
- Create `autolight/timeline/model.py`: QML-facing `QAbstractListModel` for tracks and marker counts.
- Create `autolight/app_controller.py`: QObject bridge exposing project state and the timeline model to QML.
- Modify `main.py`: initialize the controller, expose it to QML, and add a `--smoke` mode for headless launch checks.
- Replace `UI/Main.qml`: graph-backed timeline shell with track list, marker lanes, status labels, and inspector area.
- Create tests under `tests/`: unit and smoke coverage for project schema, graph semantics, cache keys, transforms, jobs, and timeline model.
- Modify `README.md`: add current run and test commands.

## Task 1: Project Domain Models

**Files:**
- Create: `autolight/__init__.py`
- Create: `autolight/project/__init__.py`
- Create: `autolight/project/models.py`
- Test: `tests/test_project_models.py`

- [ ] **Step 1: Write the failing model tests**

Create `tests/test_project_models.py`:

```python
import unittest

from autolight.project.models import (
    AudioAsset,
    Marker,
    ProjectDocument,
    ResultState,
    Track,
    TrackType,
)


class ProjectModelsTest(unittest.TestCase):
    def test_project_defaults_are_serializable_domain_objects(self):
        project = ProjectDocument(id="project_1", name="Demo")

        self.assertEqual(project.schema_version, 1)
        self.assertEqual(project.audio_assets, [])
        self.assertEqual(project.tracks, [])
        self.assertEqual(project.markers, [])

    def test_track_uses_list_inputs_for_future_dag_compatibility(self):
        track = Track(
            id="track_pitch",
            type=TrackType.GENERATED,
            name="Pitch",
            input_track_ids=["track_vocals"],
            transform_id="pitch.basic",
            transform_version="1",
        )

        self.assertEqual(track.input_track_ids, ["track_vocals"])
        self.assertEqual(track.result_state, ResultState.PENDING)

    def test_editable_marker_keeps_source_marker_ids(self):
        marker = Marker(
            id="marker_edit_1",
            track_id="track_edit",
            timestamp=1.25,
            label="Cue",
            source_marker_ids=["marker_generated_1"],
            metadata={"color": "blue"},
        )

        self.assertEqual(marker.source_marker_ids, ["marker_generated_1"])
        self.assertEqual(marker.metadata["color"], "blue")

    def test_audio_asset_records_relinkable_source_file(self):
        asset = AudioAsset(
            id="asset_1",
            path="/music/song.wav",
            duration=12.5,
            sample_rate=44100,
            channels=2,
            fingerprint="abc123",
        )

        self.assertEqual(asset.import_status, "online")
        self.assertEqual(asset.relink_hint, "")


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the tests and verify they fail because the package is missing**

Run:

```bash
uv run python -m unittest tests.test_project_models -v
```

Expected: FAIL with `ModuleNotFoundError: No module named 'autolight'`.

- [ ] **Step 3: Create the package and domain models**

Create `autolight/__init__.py`:

```python
__version__ = "0.1.0"
```

Create `autolight/project/__init__.py`:

```python
from autolight.project.models import (
    AudioAsset,
    CacheEntry,
    JobRun,
    Marker,
    ProjectDocument,
    ResultState,
    Track,
    TrackType,
)

__all__ = [
    "AudioAsset",
    "CacheEntry",
    "JobRun",
    "Marker",
    "ProjectDocument",
    "ResultState",
    "Track",
    "TrackType",
]
```

Create `autolight/project/models.py`:

```python
from __future__ import annotations

from dataclasses import dataclass, field
from enum import StrEnum
from typing import Any


SCHEMA_VERSION = 1


class TrackType(StrEnum):
    SOURCE = "source"
    GENERATED = "generated"
    EDITABLE = "editable"


class ResultState(StrEnum):
    PENDING = "pending"
    RUNNING = "running"
    COMPLETE = "complete"
    STALE = "stale"
    FAILED = "failed"
    CANCELLED = "cancelled"
    BLOCKED = "blocked"


@dataclass(slots=True)
class AudioAsset:
    id: str
    path: str
    duration: float
    sample_rate: int
    channels: int
    fingerprint: str
    import_status: str = "online"
    relink_hint: str = ""


@dataclass(slots=True)
class Track:
    id: str
    type: TrackType
    name: str
    input_track_ids: list[str] = field(default_factory=list)
    transform_id: str = ""
    transform_params: dict[str, Any] = field(default_factory=dict)
    transform_version: str = ""
    output_schema: str = ""
    dependency_hash: str = ""
    result_state: ResultState = ResultState.PENDING
    cache_refs: list[str] = field(default_factory=list)
    provenance: dict[str, Any] = field(default_factory=dict)
    error: str = ""


@dataclass(slots=True)
class Marker:
    id: str
    track_id: str
    timestamp: float
    duration: float | None = None
    label: str = ""
    category: str = ""
    confidence: float | None = None
    tags: list[str] = field(default_factory=list)
    source_transform: str = ""
    source_marker_ids: list[str] = field(default_factory=list)
    metadata: dict[str, Any] = field(default_factory=dict)


@dataclass(slots=True)
class JobRun:
    id: str
    track_id: str
    transform_id: str
    parameters_hash: str
    state: ResultState = ResultState.PENDING
    progress: float = 0.0
    started_at: str = ""
    completed_at: str = ""
    error: str = ""
    produced_cache_refs: list[str] = field(default_factory=list)


@dataclass(slots=True)
class CacheEntry:
    id: str
    dependency_hash: str
    artifact_kind: str
    path: str
    created_at: str
    transform_version: str
    size_bytes: int = 0
    validation_status: str = "valid"


@dataclass(slots=True)
class ProjectDocument:
    id: str
    name: str
    schema_version: int = SCHEMA_VERSION
    audio_assets: list[AudioAsset] = field(default_factory=list)
    tracks: list[Track] = field(default_factory=list)
    markers: list[Marker] = field(default_factory=list)
    job_runs: list[JobRun] = field(default_factory=list)
    cache_entries: list[CacheEntry] = field(default_factory=list)
    ui_state: dict[str, Any] = field(default_factory=dict)
```

- [ ] **Step 4: Run the model tests and verify they pass**

Run:

```bash
uv run python -m unittest tests.test_project_models -v
```

Expected: PASS with 4 tests.

- [ ] **Step 5: Commit the domain models**

Run:

```bash
git add autolight/__init__.py autolight/project/__init__.py autolight/project/models.py tests/test_project_models.py
git commit -m "Add project domain models"
```

Expected: commit succeeds.

## Task 2: Project Store And Graph Semantics

**Files:**
- Create: `autolight/project/store.py`
- Modify: `autolight/project/__init__.py`
- Test: `tests/test_project_store.py`

- [ ] **Step 1: Write failing store and graph tests**

Create `tests/test_project_store.py`:

```python
import tempfile
import unittest
from pathlib import Path

from autolight.project.models import ResultState, TrackType
from autolight.project.store import (
    ProjectStore,
    add_generated_track,
    create_editable_track_from_markers,
    import_audio_asset,
    mark_dependents_stale,
    new_project,
    validate_graph,
)


class ProjectStoreTest(unittest.TestCase):
    def test_save_and_load_project_round_trip(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            audio_path.write_bytes(b"fake audio bytes")
            project_path = Path(tmp) / "show.autolight"

            project = new_project("Demo")
            source_track = import_audio_asset(project, audio_path)
            generated = add_generated_track(
                project,
                parent_track_id=source_track.id,
                name="Beat Markers",
                transform_id="markers.beats",
                transform_params={"interval": 0.5},
                transform_version="1",
                output_schema="markers.v1",
                dependency_hash="hash_1",
            )

            ProjectStore.save(project, project_path)
            loaded = ProjectStore.load(project_path)

            self.assertEqual(loaded.name, "Demo")
            self.assertEqual(loaded.audio_assets[0].fingerprint, project.audio_assets[0].fingerprint)
            self.assertEqual(loaded.tracks[1].id, generated.id)
            self.assertEqual(loaded.tracks[1].type, TrackType.GENERATED)

    def test_single_parent_validation_rejects_generated_track_with_two_inputs(self):
        project = new_project("Demo")
        project.tracks.append(
            add_generated_track(
                project,
                parent_track_id="missing_parent",
                name="Invalid",
                transform_id="x",
                transform_params={},
                transform_version="1",
                output_schema="markers.v1",
                dependency_hash="hash",
                validate_parent=False,
            )
        )
        project.tracks[-1].input_track_ids = ["a", "b"]

        with self.assertRaisesRegex(ValueError, "exactly one input"):
            validate_graph(project)

    def test_stale_propagation_marks_generated_descendants_only(self):
        project = new_project("Demo")
        source = import_audio_asset_from_bytes(project, b"audio")
        beat = add_generated_track(project, source.id, "Beats", "markers.beats", {}, "1", "markers.v1", "h1")
        edit = create_editable_track_from_markers(project, beat.id, "Edited Beats", [])
        pitch = add_generated_track(project, beat.id, "Pitch", "pitch.basic", {}, "1", "markers.v1", "h2")
        beat.result_state = ResultState.COMPLETE
        edit.result_state = ResultState.COMPLETE
        pitch.result_state = ResultState.COMPLETE

        mark_dependents_stale(project, beat.id)

        self.assertEqual(edit.result_state, ResultState.COMPLETE)
        self.assertEqual(pitch.result_state, ResultState.STALE)


def import_audio_asset_from_bytes(project, payload: bytes):
    with tempfile.TemporaryDirectory() as tmp:
        audio_path = Path(tmp) / "song.wav"
        audio_path.write_bytes(payload)
        return import_audio_asset(project, audio_path)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the store tests and verify they fail because `store.py` is missing**

Run:

```bash
uv run python -m unittest tests.test_project_store -v
```

Expected: FAIL with `ModuleNotFoundError` or import errors for `autolight.project.store`.

- [ ] **Step 3: Implement project store, graph helpers, and serialization**

Create `autolight/project/store.py`:

```python
from __future__ import annotations

import hashlib
import json
from dataclasses import asdict, is_dataclass
from enum import StrEnum
from pathlib import Path
from typing import Any
from uuid import uuid4

from autolight.project.models import (
    AudioAsset,
    CacheEntry,
    JobRun,
    Marker,
    ProjectDocument,
    ResultState,
    Track,
    TrackType,
)


def new_id(prefix: str) -> str:
    return f"{prefix}_{uuid4().hex[:12]}"


def new_project(name: str) -> ProjectDocument:
    return ProjectDocument(id=new_id("project"), name=name)


def fingerprint_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def import_audio_asset(project: ProjectDocument, path: str | Path) -> Track:
    audio_path = Path(path)
    if not audio_path.exists():
        raise FileNotFoundError(str(audio_path))

    asset = AudioAsset(
        id=new_id("asset"),
        path=str(audio_path),
        duration=0.0,
        sample_rate=0,
        channels=0,
        fingerprint=fingerprint_file(audio_path),
    )
    track = Track(
        id=new_id("track"),
        type=TrackType.SOURCE,
        name=audio_path.stem,
        result_state=ResultState.COMPLETE,
        provenance={"asset_id": asset.id},
    )
    project.audio_assets.append(asset)
    project.tracks.append(track)
    return track


def add_generated_track(
    project: ProjectDocument,
    parent_track_id: str,
    name: str,
    transform_id: str,
    transform_params: dict[str, Any],
    transform_version: str,
    output_schema: str,
    dependency_hash: str,
    validate_parent: bool = True,
) -> Track:
    if validate_parent and find_track(project, parent_track_id) is None:
        raise ValueError(f"parent track not found: {parent_track_id}")

    track = Track(
        id=new_id("track"),
        type=TrackType.GENERATED,
        name=name,
        input_track_ids=[parent_track_id],
        transform_id=transform_id,
        transform_params=dict(transform_params),
        transform_version=transform_version,
        output_schema=output_schema,
        dependency_hash=dependency_hash,
    )
    if validate_parent:
        project.tracks.append(track)
    return track


def create_editable_track_from_markers(
    project: ProjectDocument,
    source_track_id: str,
    name: str,
    source_marker_ids: list[str],
) -> Track:
    if find_track(project, source_track_id) is None:
        raise ValueError(f"source track not found: {source_track_id}")

    track = Track(
        id=new_id("track"),
        type=TrackType.EDITABLE,
        name=name,
        input_track_ids=[source_track_id],
        result_state=ResultState.COMPLETE,
        provenance={"source_track_id": source_track_id, "source_marker_ids": list(source_marker_ids)},
    )
    project.tracks.append(track)
    return track


def find_track(project: ProjectDocument, track_id: str) -> Track | None:
    return next((track for track in project.tracks if track.id == track_id), None)


def validate_graph(project: ProjectDocument) -> None:
    track_ids = {track.id for track in project.tracks}
    if len(track_ids) != len(project.tracks):
        raise ValueError("duplicate track id")

    for track in project.tracks:
        if track.type == TrackType.SOURCE and track.input_track_ids:
            raise ValueError("source tracks cannot have inputs")
        if track.type == TrackType.GENERATED and len(track.input_track_ids) != 1:
            raise ValueError("generated tracks must have exactly one input")
        for input_id in track.input_track_ids:
            if input_id not in track_ids:
                raise ValueError(f"missing input track: {input_id}")


def mark_dependents_stale(project: ProjectDocument, changed_track_id: str) -> None:
    changed = True
    stale_ids = {changed_track_id}
    while changed:
        changed = False
        for track in project.tracks:
            if track.type != TrackType.GENERATED:
                continue
            if track.id in stale_ids:
                continue
            if any(input_id in stale_ids for input_id in track.input_track_ids):
                track.result_state = ResultState.STALE
                stale_ids.add(track.id)
                changed = True


class ProjectStore:
    @staticmethod
    def save(project: ProjectDocument, path: str | Path) -> None:
        validate_graph(project)
        target = Path(path)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(_to_json(project), indent=2, sort_keys=True), encoding="utf-8")

    @staticmethod
    def load(path: str | Path) -> ProjectDocument:
        raw = json.loads(Path(path).read_text(encoding="utf-8"))
        return _project_from_json(raw)


def _to_json(value: Any) -> Any:
    if isinstance(value, StrEnum):
        return value.value
    if is_dataclass(value):
        return {key: _to_json(item) for key, item in asdict(value).items()}
    if isinstance(value, list):
        return [_to_json(item) for item in value]
    if isinstance(value, dict):
        return {key: _to_json(item) for key, item in value.items()}
    return value


def _project_from_json(raw: dict[str, Any]) -> ProjectDocument:
    return ProjectDocument(
        id=raw["id"],
        name=raw["name"],
        schema_version=raw["schema_version"],
        audio_assets=[AudioAsset(**item) for item in raw["audio_assets"]],
        tracks=[
            Track(
                **{
                    **item,
                    "type": TrackType(item["type"]),
                    "result_state": ResultState(item["result_state"]),
                }
            )
            for item in raw["tracks"]
        ],
        markers=[Marker(**item) for item in raw["markers"]],
        job_runs=[
            JobRun(
                **{
                    **item,
                    "state": ResultState(item["state"]),
                }
            )
            for item in raw["job_runs"]
        ],
        cache_entries=[CacheEntry(**item) for item in raw["cache_entries"]],
        ui_state=raw["ui_state"],
    )
```

Modify `autolight/project/__init__.py`:

```python
from autolight.project.models import (
    AudioAsset,
    CacheEntry,
    JobRun,
    Marker,
    ProjectDocument,
    ResultState,
    Track,
    TrackType,
)
from autolight.project.store import (
    ProjectStore,
    add_generated_track,
    create_editable_track_from_markers,
    find_track,
    import_audio_asset,
    mark_dependents_stale,
    new_project,
    validate_graph,
)

__all__ = [
    "AudioAsset",
    "CacheEntry",
    "JobRun",
    "Marker",
    "ProjectDocument",
    "ProjectStore",
    "ResultState",
    "Track",
    "TrackType",
    "add_generated_track",
    "create_editable_track_from_markers",
    "find_track",
    "import_audio_asset",
    "mark_dependents_stale",
    "new_project",
    "validate_graph",
]
```

- [ ] **Step 4: Run project tests**

Run:

```bash
uv run python -m unittest tests.test_project_models tests.test_project_store -v
```

Expected: PASS with all project tests.

- [ ] **Step 5: Commit project store and graph semantics**

Run:

```bash
git add autolight/project/__init__.py autolight/project/store.py tests/test_project_store.py
git commit -m "Add project store and track graph semantics"
```

Expected: commit succeeds.

## Task 3: Cache Keys And Artifact Store

**Files:**
- Create: `autolight/cache/__init__.py`
- Create: `autolight/cache/keys.py`
- Create: `autolight/cache/store.py`
- Test: `tests/test_cache.py`

- [ ] **Step 1: Write failing cache tests**

Create `tests/test_cache.py`:

```python
import tempfile
import unittest
from pathlib import Path

from autolight.cache.keys import canonical_hash, track_dependency_hash
from autolight.cache.store import CacheStore


class CacheTest(unittest.TestCase):
    def test_canonical_hash_is_order_stable(self):
        left = canonical_hash({"b": 2, "a": 1})
        right = canonical_hash({"a": 1, "b": 2})

        self.assertEqual(left, right)

    def test_track_dependency_hash_includes_parent_transform_and_params(self):
        first = track_dependency_hash(
            input_cache_refs=["audio:abc"],
            transform_id="markers.beats",
            transform_version="1",
            params={"interval": 0.5},
        )
        second = track_dependency_hash(
            input_cache_refs=["audio:abc"],
            transform_id="markers.beats",
            transform_version="2",
            params={"interval": 0.5},
        )

        self.assertNotEqual(first, second)

    def test_cache_store_writes_artifact_and_reports_valid_entry(self):
        with tempfile.TemporaryDirectory() as tmp:
            store = CacheStore(Path(tmp))
            entry = store.write_bytes("markers", "dep_hash", b"[]", "1")

            self.assertTrue(store.artifact_path(entry).exists())
            self.assertTrue(store.is_entry_valid(entry))
            self.assertEqual(entry.artifact_kind, "markers")


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run cache tests and verify they fail because cache modules are missing**

Run:

```bash
uv run python -m unittest tests.test_cache -v
```

Expected: FAIL with `ModuleNotFoundError: No module named 'autolight.cache'`.

- [ ] **Step 3: Implement stable cache keys and local artifact writes**

Create `autolight/cache/__init__.py`:

```python
from autolight.cache.keys import canonical_hash, track_dependency_hash
from autolight.cache.store import CacheStore

__all__ = ["CacheStore", "canonical_hash", "track_dependency_hash"]
```

Create `autolight/cache/keys.py`:

```python
from __future__ import annotations

import hashlib
import json
from typing import Any


def canonical_hash(payload: Any) -> str:
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":"), default=str).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def track_dependency_hash(
    input_cache_refs: list[str],
    transform_id: str,
    transform_version: str,
    params: dict[str, Any],
) -> str:
    return canonical_hash(
        {
            "input_cache_refs": input_cache_refs,
            "transform_id": transform_id,
            "transform_version": transform_version,
            "params": params,
        }
    )
```

Create `autolight/cache/store.py`:

```python
from __future__ import annotations

from datetime import datetime, timezone
from pathlib import Path

from autolight.cache.keys import canonical_hash
from autolight.project.models import CacheEntry


class CacheStore:
    def __init__(self, root: Path):
        self.root = root
        self.root.mkdir(parents=True, exist_ok=True)

    def write_bytes(
        self,
        artifact_kind: str,
        dependency_hash: str,
        payload: bytes,
        transform_version: str,
    ) -> CacheEntry:
        entry_id = canonical_hash({"kind": artifact_kind, "dependency": dependency_hash, "payload": payload.hex()})[:16]
        relative_path = Path(artifact_kind) / f"{entry_id}.bin"
        target = self.root / relative_path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(payload)
        return CacheEntry(
            id=entry_id,
            dependency_hash=dependency_hash,
            artifact_kind=artifact_kind,
            path=str(relative_path),
            created_at=datetime.now(timezone.utc).isoformat(),
            transform_version=transform_version,
            size_bytes=len(payload),
        )

    def artifact_path(self, entry: CacheEntry) -> Path:
        return self.root / entry.path

    def is_entry_valid(self, entry: CacheEntry) -> bool:
        path = self.artifact_path(entry)
        return path.exists() and path.stat().st_size == entry.size_bytes
```

- [ ] **Step 4: Run cache tests**

Run:

```bash
uv run python -m unittest tests.test_cache -v
```

Expected: PASS with 3 tests.

- [ ] **Step 5: Commit cache support**

Run:

```bash
git add autolight/cache tests/test_cache.py
git commit -m "Add cache key and artifact store"
```

Expected: commit succeeds.

## Task 4: Transform Registry And Built-In Stand-In Transforms

**Files:**
- Create: `autolight/analysis/__init__.py`
- Create: `autolight/analysis/registry.py`
- Create: `autolight/analysis/builtin.py`
- Test: `tests/test_analysis.py`

- [ ] **Step 1: Write failing analysis tests**

Create `tests/test_analysis.py`:

```python
import tempfile
import unittest
from pathlib import Path

from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformContext, TransformRegistry


class AnalysisRegistryTest(unittest.TestCase):
    def test_builtin_registry_contains_marker_and_expensive_transforms(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        self.assertIn("markers.fixed_interval", registry.ids())
        self.assertIn("stems.vocals_stand_in", registry.ids())

    def test_fixed_interval_transform_returns_markers(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        transform = registry.get("markers.fixed_interval")

        with tempfile.TemporaryDirectory() as tmp:
            result = transform.run(
                TransformContext(artifact_dir=Path(tmp), cancel_requested=lambda: False, progress=lambda value: None),
                {"duration": 2.0, "interval": 0.5},
            )

        self.assertEqual([marker["timestamp"] for marker in result.markers], [0.0, 0.5, 1.0, 1.5, 2.0])
        self.assertEqual(result.artifacts, {})

    def test_vocal_stand_in_writes_artifact(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        transform = registry.get("stems.vocals_stand_in")
        progress_values = []

        with tempfile.TemporaryDirectory() as tmp:
            result = transform.run(
                TransformContext(artifact_dir=Path(tmp), cancel_requested=lambda: False, progress=progress_values.append),
                {"label": "vocals"},
            )

        self.assertEqual(progress_values[-1], 1.0)
        self.assertTrue(Path(result.artifacts["stem"]).exists())


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run analysis tests and verify they fail because analysis modules are missing**

Run:

```bash
uv run python -m unittest tests.test_analysis -v
```

Expected: FAIL with `ModuleNotFoundError: No module named 'autolight.analysis'`.

- [ ] **Step 3: Implement transform registry contracts**

Create `autolight/analysis/__init__.py`:

```python
from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformContext, TransformRegistry, TransformResult, TransformSpec

__all__ = [
    "TransformContext",
    "TransformRegistry",
    "TransformResult",
    "TransformSpec",
    "register_builtin_transforms",
]
```

Create `autolight/analysis/registry.py`:

```python
from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable


ProgressCallback = Callable[[float], None]
CancelPredicate = Callable[[], bool]
TransformRunner = Callable[["TransformContext", dict[str, Any]], "TransformResult"]


@dataclass(slots=True)
class TransformContext:
    artifact_dir: Path
    cancel_requested: CancelPredicate
    progress: ProgressCallback


@dataclass(slots=True)
class TransformResult:
    markers: list[dict[str, Any]] = field(default_factory=list)
    artifacts: dict[str, str] = field(default_factory=dict)
    metadata: dict[str, Any] = field(default_factory=dict)


@dataclass(slots=True)
class TransformSpec:
    id: str
    version: str
    name: str
    input_schema: str
    output_schema: str
    estimated_cost: str
    run: TransformRunner


class TransformRegistry:
    def __init__(self):
        self._transforms: dict[str, TransformSpec] = {}

    def register(self, spec: TransformSpec) -> None:
        if spec.id in self._transforms:
            raise ValueError(f"duplicate transform id: {spec.id}")
        self._transforms[spec.id] = spec

    def get(self, transform_id: str) -> TransformSpec:
        try:
            return self._transforms[transform_id]
        except KeyError as exc:
            raise KeyError(f"unknown transform id: {transform_id}") from exc

    def ids(self) -> list[str]:
        return sorted(self._transforms)
```

- [ ] **Step 4: Implement deterministic built-in transforms**

Create `autolight/analysis/builtin.py`:

```python
from __future__ import annotations

import json
import time
from pathlib import Path

from autolight.analysis.registry import TransformContext, TransformRegistry, TransformResult, TransformSpec


def register_builtin_transforms(registry: TransformRegistry) -> None:
    registry.register(
        TransformSpec(
            id="markers.fixed_interval",
            version="1",
            name="Fixed Interval Markers",
            input_schema="audio-or-markers.v1",
            output_schema="markers.v1",
            estimated_cost="light",
            run=_fixed_interval_markers,
        )
    )
    registry.register(
        TransformSpec(
            id="stems.vocals_stand_in",
            version="1",
            name="Vocals Stem Stand-In",
            input_schema="audio.v1",
            output_schema="artifact.stem.v1",
            estimated_cost="heavy",
            run=_vocals_stand_in,
        )
    )


def _fixed_interval_markers(context: TransformContext, params: dict) -> TransformResult:
    duration = float(params.get("duration", 0.0))
    interval = float(params.get("interval", 1.0))
    if interval <= 0:
        raise ValueError("interval must be greater than zero")

    markers = []
    current = 0.0
    while current <= duration + 1e-9:
        if context.cancel_requested():
            raise RuntimeError("cancelled")
        markers.append(
            {
                "timestamp": round(current, 6),
                "label": "Beat",
                "category": "timing",
                "confidence": 1.0,
                "metadata": {"interval": interval},
            }
        )
        current += interval
    context.progress(1.0)
    return TransformResult(markers=markers)


def _vocals_stand_in(context: TransformContext, params: dict) -> TransformResult:
    label = str(params.get("label", "vocals"))
    context.artifact_dir.mkdir(parents=True, exist_ok=True)
    for step in range(1, 4):
        if context.cancel_requested():
            raise RuntimeError("cancelled")
        context.progress(step / 4)
        time.sleep(0.01)

    artifact = Path(context.artifact_dir) / f"{label}.json"
    artifact.write_text(json.dumps({"stem": label, "samples": []}, sort_keys=True), encoding="utf-8")
    context.progress(1.0)
    return TransformResult(artifacts={"stem": str(artifact)}, metadata={"stem": label})
```

- [ ] **Step 5: Run analysis tests**

Run:

```bash
uv run python -m unittest tests.test_analysis -v
```

Expected: PASS with 3 tests.

- [ ] **Step 6: Commit analysis registry**

Run:

```bash
git add autolight/analysis tests/test_analysis.py
git commit -m "Add transform registry and built-in transforms"
```

Expected: commit succeeds.

## Task 5: Local Background Job Queue

**Files:**
- Create: `autolight/jobs/__init__.py`
- Create: `autolight/jobs/queue.py`
- Test: `tests/test_jobs.py`

- [ ] **Step 1: Write failing job queue tests**

Create `tests/test_jobs.py`:

```python
import tempfile
import time
import unittest
from pathlib import Path

from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformRegistry
from autolight.jobs.queue import LocalJobQueue
from autolight.project.models import ResultState
from autolight.project.store import add_generated_track, import_audio_asset, new_project


class LocalJobQueueTest(unittest.TestCase):
    def test_successful_job_marks_track_complete_and_adds_markers(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        with tempfile.TemporaryDirectory() as tmp:
            project, track_id = project_with_generated_track(Path(tmp), "markers.fixed_interval", {"duration": 1.0, "interval": 0.5})
            queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")
            job_id = queue.submit(project, track_id)

            queue.wait(job_id, timeout=2)

        track = next(track for track in project.tracks if track.id == track_id)
        self.assertEqual(track.result_state, ResultState.COMPLETE)
        self.assertEqual(len([marker for marker in project.markers if marker.track_id == track_id]), 3)

    def test_failed_job_keeps_track_and_records_error(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        with tempfile.TemporaryDirectory() as tmp:
            project, track_id = project_with_generated_track(Path(tmp), "markers.fixed_interval", {"interval": 0})
            queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")
            job_id = queue.submit(project, track_id)

            queue.wait(job_id, timeout=2)

        track = next(track for track in project.tracks if track.id == track_id)
        self.assertEqual(track.result_state, ResultState.FAILED)
        self.assertIn("interval", track.error)

    def test_cancelled_job_does_not_mark_track_complete(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        with tempfile.TemporaryDirectory() as tmp:
            project, track_id = project_with_generated_track(Path(tmp), "stems.vocals_stand_in", {"label": "vocals"})
            queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")
            job_id = queue.submit(project, track_id)
            time.sleep(0.005)
            queue.cancel(job_id)
            queue.wait(job_id, timeout=2)

        track = next(track for track in project.tracks if track.id == track_id)
        self.assertIn(track.result_state, {ResultState.CANCELLED, ResultState.FAILED})
        self.assertEqual([marker for marker in project.markers if marker.track_id == track_id], [])


def project_with_generated_track(tmp: Path, transform_id: str, params: dict):
    audio_path = tmp / "song.wav"
    audio_path.write_bytes(b"audio")
    project = new_project("Demo")
    source = import_audio_asset(project, audio_path)
    generated = add_generated_track(
        project,
        parent_track_id=source.id,
        name="Generated",
        transform_id=transform_id,
        transform_params=params,
        transform_version="1",
        output_schema="markers.v1",
        dependency_hash="dep",
    )
    return project, generated.id


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run job tests and verify they fail because jobs module is missing**

Run:

```bash
uv run python -m unittest tests.test_jobs -v
```

Expected: FAIL with `ModuleNotFoundError: No module named 'autolight.jobs'`.

- [ ] **Step 3: Implement the local job queue**

Create `autolight/jobs/__init__.py`:

```python
from autolight.jobs.queue import LocalJobQueue

__all__ = ["LocalJobQueue"]
```

Create `autolight/jobs/queue.py`:

```python
from __future__ import annotations

from concurrent.futures import Future, ThreadPoolExecutor
from datetime import datetime, timezone
from pathlib import Path
from threading import Event

from autolight.analysis.registry import TransformContext, TransformRegistry
from autolight.project.models import JobRun, Marker, ProjectDocument, ResultState, Track
from autolight.project.store import find_track, new_id


class LocalJobQueue:
    def __init__(self, registry: TransformRegistry, artifact_root: Path):
        self.registry = registry
        self.artifact_root = artifact_root
        self.artifact_root.mkdir(parents=True, exist_ok=True)
        self._executor = ThreadPoolExecutor(max_workers=2)
        self._futures: dict[str, Future] = {}
        self._cancel_events: dict[str, Event] = {}

    def submit(self, project: ProjectDocument, track_id: str) -> str:
        track = find_track(project, track_id)
        if track is None:
            raise ValueError(f"track not found: {track_id}")
        if not track.transform_id:
            raise ValueError("track has no transform")

        job_id = new_id("job")
        cancel_event = Event()
        run = JobRun(
            id=job_id,
            track_id=track_id,
            transform_id=track.transform_id,
            parameters_hash=track.dependency_hash,
            state=ResultState.RUNNING,
            started_at=datetime.now(timezone.utc).isoformat(),
        )
        project.job_runs.append(run)
        track.result_state = ResultState.RUNNING
        track.error = ""

        future = self._executor.submit(self._run, project, track, run, cancel_event)
        self._futures[job_id] = future
        self._cancel_events[job_id] = cancel_event
        return job_id

    def cancel(self, job_id: str) -> None:
        event = self._cancel_events.get(job_id)
        if event is not None:
            event.set()

    def wait(self, job_id: str, timeout: float | None = None) -> None:
        self._futures[job_id].result(timeout=timeout)

    def shutdown(self) -> None:
        self._executor.shutdown(wait=True)

    def _run(self, project: ProjectDocument, track: Track, run: JobRun, cancel_event: Event) -> None:
        transform = self.registry.get(track.transform_id)
        artifact_dir = self.artifact_root / run.id

        def progress(value: float) -> None:
            run.progress = max(0.0, min(1.0, value))

        context = TransformContext(artifact_dir=artifact_dir, cancel_requested=cancel_event.is_set, progress=progress)
        try:
            result = transform.run(context, track.transform_params)
            if cancel_event.is_set():
                track.result_state = ResultState.CANCELLED
                run.state = ResultState.CANCELLED
                return
            for item in result.markers:
                project.markers.append(
                    Marker(
                        id=new_id("marker"),
                        track_id=track.id,
                        timestamp=float(item["timestamp"]),
                        label=str(item.get("label", "")),
                        category=str(item.get("category", "")),
                        confidence=item.get("confidence"),
                        source_transform=track.transform_id,
                        metadata=dict(item.get("metadata", {})),
                    )
                )
            track.cache_refs = list(result.artifacts.values())
            track.result_state = ResultState.COMPLETE
            run.state = ResultState.COMPLETE
            run.progress = 1.0
            run.produced_cache_refs = list(result.artifacts.values())
        except Exception as exc:
            if cancel_event.is_set() or str(exc) == "cancelled":
                track.result_state = ResultState.CANCELLED
                run.state = ResultState.CANCELLED
            else:
                track.result_state = ResultState.FAILED
                run.state = ResultState.FAILED
                track.error = str(exc)
                run.error = str(exc)
        finally:
            run.completed_at = datetime.now(timezone.utc).isoformat()
```

- [ ] **Step 4: Run job tests**

Run:

```bash
uv run python -m unittest tests.test_jobs -v
```

Expected: PASS with 3 tests.

- [ ] **Step 5: Commit job queue**

Run:

```bash
git add autolight/jobs tests/test_jobs.py
git commit -m "Add local background job queue"
```

Expected: commit succeeds.

## Task 6: Timeline View Model

**Files:**
- Create: `autolight/timeline/__init__.py`
- Create: `autolight/timeline/model.py`
- Test: `tests/test_timeline_model.py`

- [ ] **Step 1: Write failing timeline model tests**

Create `tests/test_timeline_model.py`:

```python
import tempfile
import unittest
from pathlib import Path

from PySide6.QtCore import QCoreApplication, Qt

from autolight.project.models import Marker, ResultState
from autolight.project.store import add_generated_track, import_audio_asset, new_project
from autolight.timeline.model import TimelineTrackModel


class TimelineTrackModelTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication([])

    def test_model_exposes_track_roles_for_qml(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            audio_path.write_bytes(b"audio")
            project = new_project("Demo")
            source = import_audio_asset(project, audio_path)
            generated = add_generated_track(project, source.id, "Beats", "markers.fixed_interval", {}, "1", "markers.v1", "dep")
            generated.result_state = ResultState.COMPLETE
            project.markers.append(Marker(id="marker_1", track_id=generated.id, timestamp=0.5))

            model = TimelineTrackModel()
            model.set_project(project)
            index = model.index(1, 0)

            self.assertEqual(model.rowCount(), 2)
            self.assertEqual(model.data(index, model.role_for_name("name")), "Beats")
            self.assertEqual(model.data(index, model.role_for_name("markerCount")), 1)
            self.assertEqual(model.data(index, model.role_for_name("resultState")), "complete")
            self.assertEqual(model.data(index, Qt.ItemDataRole.DisplayRole), "Beats")


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run timeline model tests and verify they fail because timeline module is missing**

Run:

```bash
uv run python -m unittest tests.test_timeline_model -v
```

Expected: FAIL with `ModuleNotFoundError: No module named 'autolight.timeline'`.

- [ ] **Step 3: Implement QML-facing track model**

Create `autolight/timeline/__init__.py`:

```python
from autolight.timeline.model import TimelineTrackModel

__all__ = ["TimelineTrackModel"]
```

Create `autolight/timeline/model.py`:

```python
from __future__ import annotations

from PySide6.QtCore import QAbstractListModel, QModelIndex, Qt

from autolight.project.models import ProjectDocument


class TimelineTrackModel(QAbstractListModel):
    ROLE_NAMES = {
        Qt.ItemDataRole.UserRole + 1: b"trackId",
        Qt.ItemDataRole.UserRole + 2: b"name",
        Qt.ItemDataRole.UserRole + 3: b"trackType",
        Qt.ItemDataRole.UserRole + 4: b"resultState",
        Qt.ItemDataRole.UserRole + 5: b"markerCount",
        Qt.ItemDataRole.UserRole + 6: b"error",
    }

    def __init__(self):
        super().__init__()
        self._project: ProjectDocument | None = None

    def set_project(self, project: ProjectDocument) -> None:
        self.beginResetModel()
        self._project = project
        self.endResetModel()

    def rowCount(self, parent: QModelIndex = QModelIndex()) -> int:
        if parent.isValid() or self._project is None:
            return 0
        return len(self._project.tracks)

    def data(self, index: QModelIndex, role: int = Qt.ItemDataRole.DisplayRole):
        if self._project is None or not index.isValid():
            return None
        track = self._project.tracks[index.row()]
        if role == Qt.ItemDataRole.DisplayRole:
            return track.name
        if role == self.role_for_name("trackId"):
            return track.id
        if role == self.role_for_name("name"):
            return track.name
        if role == self.role_for_name("trackType"):
            return track.type.value
        if role == self.role_for_name("resultState"):
            return track.result_state.value
        if role == self.role_for_name("markerCount"):
            return len([marker for marker in self._project.markers if marker.track_id == track.id])
        if role == self.role_for_name("error"):
            return track.error
        return None

    def roleNames(self):
        return self.ROLE_NAMES

    def role_for_name(self, name: str) -> int:
        encoded = name.encode("utf-8")
        for role, role_name in self.ROLE_NAMES.items():
            if role_name == encoded:
                return role
        raise KeyError(name)
```

- [ ] **Step 4: Run timeline model tests**

Run:

```bash
uv run python -m unittest tests.test_timeline_model -v
```

Expected: PASS with 1 test.

- [ ] **Step 5: Commit timeline model**

Run:

```bash
git add autolight/timeline tests/test_timeline_model.py
git commit -m "Add timeline track model"
```

Expected: commit succeeds.

## Task 7: App Controller And QML Timeline Shell

**Files:**
- Create: `autolight/app_controller.py`
- Modify: `main.py`
- Replace: `UI/Main.qml`
- Test: `tests/test_app_controller.py`

- [ ] **Step 1: Write failing app controller tests**

Create `tests/test_app_controller.py`:

```python
import unittest

from PySide6.QtCore import QCoreApplication

from autolight.app_controller import AppController


class AppControllerTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication([])

    def test_controller_loads_demo_project_into_timeline_model(self):
        controller = AppController()

        controller.load_demo_project()

        self.assertGreaterEqual(controller.trackModel.rowCount(), 2)
        self.assertEqual(controller.projectName, "Autolight Demo")


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run controller tests and verify they fail because controller is missing**

Run:

```bash
uv run python -m unittest tests.test_app_controller -v
```

Expected: FAIL with `ModuleNotFoundError: No module named 'autolight.app_controller'`.

- [ ] **Step 3: Implement the controller**

Create `autolight/app_controller.py`:

```python
from __future__ import annotations

import tempfile
from pathlib import Path

from PySide6.QtCore import Property, QObject, Slot

from autolight.project.models import Marker, ResultState
from autolight.project.store import add_generated_track, create_editable_track_from_markers, import_audio_asset, new_project
from autolight.timeline.model import TimelineTrackModel


class AppController(QObject):
    def __init__(self):
        super().__init__()
        self._project = new_project("Untitled")
        self._track_model = TimelineTrackModel()
        self._track_model.set_project(self._project)

    @Property(QObject, constant=True)
    def trackModel(self):
        return self._track_model

    @Property(str)
    def projectName(self) -> str:
        return self._project.name

    @Slot()
    def load_demo_project(self) -> None:
        tmp = Path(tempfile.gettempdir()) / "autolight-demo-song.wav"
        tmp.write_bytes(b"demo audio")
        self._project = new_project("Autolight Demo")
        source = import_audio_asset(self._project, tmp)
        beats = add_generated_track(
            self._project,
            parent_track_id=source.id,
            name="Beat Markers",
            transform_id="markers.fixed_interval",
            transform_params={"duration": 2.0, "interval": 0.5},
            transform_version="1",
            output_schema="markers.v1",
            dependency_hash="demo",
        )
        beats.result_state = ResultState.COMPLETE
        self._project.markers.extend(
            [
                Marker(id="marker_demo_1", track_id=beats.id, timestamp=0.0, label="Beat"),
                Marker(id="marker_demo_2", track_id=beats.id, timestamp=0.5, label="Beat"),
                Marker(id="marker_demo_3", track_id=beats.id, timestamp=1.0, label="Beat"),
            ]
        )
        create_editable_track_from_markers(self._project, beats.id, "Editable Cues", ["marker_demo_1", "marker_demo_2"])
        self._track_model.set_project(self._project)
```

- [ ] **Step 4: Modify `main.py` to expose the controller and smoke mode**

Replace `main.py`:

```python
import sys

from PySide6.QtGui import QGuiApplication
from PySide6.QtQml import QQmlApplicationEngine

from autolight.app_controller import AppController


def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv if argv is None else argv)
    app = QGuiApplication(args)
    controller = AppController()
    controller.load_demo_project()

    if "--smoke" in args:
        return 0

    engine = QQmlApplicationEngine()
    engine.rootContext().setContextProperty("appController", controller)
    engine.addImportPath(sys.path[0])
    engine.loadFromModule("UI", "Main")
    if not engine.rootObjects():
        return -1
    exit_code = app.exec()
    del engine
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
```

- [ ] **Step 5: Replace `UI/Main.qml` with the timeline shell**

Replace `UI/Main.qml`:

```qml
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Window {
    width: 1120
    height: 720
    visible: true
    title: appController.projectName
    color: "#181a1f"

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        ToolBar {
            Layout.fillWidth: true

            RowLayout {
                anchors.fill: parent
                spacing: 12

                Label {
                    text: appController.projectName
                    color: "#f4f4f5"
                    font.pixelSize: 16
                    font.bold: true
                    Layout.leftMargin: 12
                }

                Item { Layout.fillWidth: true }

                Button {
                    text: "Load Demo"
                    onClicked: appController.load_demo_project()
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            ListView {
                id: trackList
                Layout.preferredWidth: 280
                Layout.fillHeight: true
                model: appController.trackModel
                clip: true

                delegate: Rectangle {
                    width: trackList.width
                    height: 74
                    color: index % 2 === 0 ? "#23262d" : "#1f2229"
                    border.color: "#343842"

                    Column {
                        anchors.fill: parent
                        anchors.margins: 10
                        spacing: 4

                        Text {
                            text: name
                            color: "#f4f4f5"
                            font.pixelSize: 14
                            elide: Text.ElideRight
                            width: parent.width
                        }

                        Text {
                            text: trackType + " - " + resultState + " - " + markerCount + " markers"
                            color: resultState === "failed" ? "#f87171" : "#a1a1aa"
                            font.pixelSize: 12
                            elide: Text.ElideRight
                            width: parent.width
                        }
                    }
                }
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                color: "#111318"

                ColumnLayout {
                    anchors.fill: parent
                    spacing: 0

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 42
                        color: "#1c1f26"

                        Row {
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.left: parent.left
                            anchors.leftMargin: 16
                            spacing: 48

                            Repeater {
                                model: 9
                                Text {
                                    text: index + "s"
                                    color: "#a1a1aa"
                                    font.pixelSize: 12
                                }
                            }
                        }
                    }

                    ListView {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        model: appController.trackModel
                        clip: true

                        delegate: Rectangle {
                            width: ListView.view.width
                            height: 74
                            color: index % 2 === 0 ? "#171a20" : "#14171d"
                            border.color: "#2f333d"

                            Repeater {
                                model: markerCount
                                Rectangle {
                                    width: 8
                                    height: parent.height - 18
                                    x: 24 + index * 48
                                    y: 9
                                    radius: 2
                                    color: trackType === "editable" ? "#67e8f9" : "#a7f3d0"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 6: Run controller and smoke checks**

Run:

```bash
uv run python -m unittest tests.test_app_controller -v
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: unittest passes and smoke command exits 0.

- [ ] **Step 7: Commit controller and QML shell**

Run:

```bash
git add autolight/app_controller.py main.py UI/Main.qml tests/test_app_controller.py
git commit -m "Add graph-backed QML timeline shell"
```

Expected: commit succeeds.

## Task 8: End-To-End Flow And Documentation

**Files:**
- Create: `tests/test_end_to_end_flow.py`
- Modify: `README.md`

- [ ] **Step 1: Write the end-to-end test**

Create `tests/test_end_to_end_flow.py`:

```python
import tempfile
import unittest
from pathlib import Path

from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformRegistry
from autolight.jobs.queue import LocalJobQueue
from autolight.project.models import ResultState
from autolight.project.store import (
    ProjectStore,
    add_generated_track,
    create_editable_track_from_markers,
    import_audio_asset,
    new_project,
)


class EndToEndFlowTest(unittest.TestCase):
    def test_import_run_derive_save_and_load(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            audio_path.write_bytes(b"audio")
            project_path = root / "show.autolight"
            project = new_project("Demo")
            source = import_audio_asset(project, audio_path)
            generated = add_generated_track(
                project,
                parent_track_id=source.id,
                name="Beat Markers",
                transform_id="markers.fixed_interval",
                transform_params={"duration": 1.0, "interval": 0.5},
                transform_version="1",
                output_schema="markers.v1",
                dependency_hash="dep",
            )
            queue = LocalJobQueue(registry, artifact_root=root / "artifacts")
            job_id = queue.submit(project, generated.id)
            queue.wait(job_id, timeout=2)

            source_marker_ids = [marker.id for marker in project.markers if marker.track_id == generated.id]
            editable = create_editable_track_from_markers(project, generated.id, "Editable Cues", source_marker_ids)
            ProjectStore.save(project, project_path)
            loaded = ProjectStore.load(project_path)

        loaded_generated = next(track for track in loaded.tracks if track.id == generated.id)
        loaded_editable = next(track for track in loaded.tracks if track.id == editable.id)
        self.assertEqual(loaded_generated.result_state, ResultState.COMPLETE)
        self.assertEqual(loaded_editable.result_state, ResultState.COMPLETE)
        self.assertEqual(len([marker for marker in loaded.markers if marker.track_id == generated.id]), 3)
        self.assertEqual(loaded_editable.provenance["source_marker_ids"], source_marker_ids)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the end-to-end test**

Run:

```bash
uv run python -m unittest tests.test_end_to_end_flow -v
```

Expected: PASS with 1 test.

- [ ] **Step 3: Update README with current run and test commands**

Replace `README.md`:

```markdown
# Autolight

Autolight is a PySide6/QML desktop app for building graph-backed audio analysis timelines. The first milestone focuses on a `.autolight` project model, generated and editable tracks, local background analysis jobs, cache-aware transform outputs, and a timeline shell.

## Run

```bash
uv run python main.py
```

For headless launch verification:

```bash
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

## Test

```bash
uv run python -m unittest discover -s tests -v
```

## Current Scope

- Import one local audio file into a project.
- Create graph-backed source, generated, and editable tracks.
- Run deterministic built-in transforms through a local background job queue.
- Persist `.autolight` project files as JSON.
- Render project tracks and marker counts in a QML timeline shell.
```

- [ ] **Step 4: Run the full verification suite**

Run:

```bash
uv run python -m unittest discover -s tests -v
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
git diff --check
```

Expected: all tests pass, smoke command exits 0, and `git diff --check` prints no errors.

- [ ] **Step 5: Commit end-to-end test and README**

Run:

```bash
git add tests/test_end_to_end_flow.py README.md
git commit -m "Document and test graph timeline flow"
```

Expected: commit succeeds.

## Final Verification

- [ ] **Step 1: Run all tests**

Run:

```bash
uv run python -m unittest discover -s tests -v
```

Expected: every test passes.

- [ ] **Step 2: Run the headless app smoke check**

Run:

```bash
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: command exits 0.

- [ ] **Step 3: Check the final diff**

Run:

```bash
git status --short
git diff --check
```

Expected: only intentional tracked changes are present before the final commit or PR, and `git diff --check` prints no errors.

## Spec Coverage

- Graph-first Python core: Tasks 1, 2, 5, and 6.
- Rich `.autolight` project fidelity: Tasks 1, 2, and 8.
- Generated read-only tracks and editable derived tracks: Tasks 1, 2, and 8.
- Single-parent implementation with list-shaped inputs: Tasks 1 and 2.
- Local background jobs: Task 5.
- Cache and dependency hashes: Task 3.
- Transform registry with source-separation-capable expensive path: Task 4.
- QML timeline projection: Tasks 6 and 7.
- Failure, cancellation, and recovery states: Tasks 2, 3, and 5.
- Testing and smoke verification: Tasks 1 through 8 plus Final Verification.
