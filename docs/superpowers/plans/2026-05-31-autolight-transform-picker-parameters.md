# Autolight Transform Picker Parameters Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the hard-coded "Add Markers" action with a controller-backed transform catalog and parameter flow.

**Architecture:** Expose registered `TransformSpec` metadata through a lightweight Qt list model. QML presents available transforms for the selected parent track and calls one generic controller slot with a transform id, version, and JSON parameter payload. Audio-backed transforms require a reachable source audio asset; the controller should derive `audio_path` from any ancestor branch and surface a clear `lastError` when no source audio is reachable.

**Tech Stack:** Python 3.14, PySide6/QML, `unittest`, existing `TransformRegistry`, `TransformSpec`, and `track_dependency_hash`.

**Prerequisite:** Complete `2026-05-31-autolight-project-workflow.md` first. This plan reuses selected-track state and controller helpers introduced there.

---

## File Structure

- Modify `autolight/analysis/registry.py`: expose registered transform specs without relying on single-version lookup.
- Create `autolight/timeline/transform_model.py`: QML list model for transform specs.
- Modify `autolight/app_controller.py`: expose `transformModel` and `add_transform_track`.
- Modify `UI/Main.qml`: add transform combo box and parameter fields.
- Create `tests/test_transform_picker.py`: model, controller, and QML wiring tests.

## Task 1: Transform Spec Model

**Files:**
- Modify: `autolight/analysis/registry.py`
- Create: `autolight/timeline/transform_model.py`
- Create: `tests/test_transform_picker.py`

- [ ] **Step 1: Write failing transform model tests**

Create `tests/test_transform_picker.py`:

```python
import unittest

from PySide6.QtCore import QCoreApplication

from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformRegistry, TransformResult, TransformSpec
from autolight.project.models import Track, TrackType
from autolight.timeline.transform_model import TransformSpecModel


class TransformPickerTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication([])

    def test_transform_model_exposes_registered_specs(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        model = TransformSpecModel(registry)

        ids = [
            model.data(model.index(row, 0), model.role_for_name("transformId"))
            for row in range(model.rowCount())
        ]

        self.assertIn("markers.fixed_interval", ids)
        self.assertIn("stems.vocals_stand_in", ids)

    def test_transform_model_exposes_multiple_versions_for_same_transform(self):
        def noop(context, params):
            return TransformResult()

        registry = TransformRegistry()
        registry.register(
            TransformSpec(
                id="test.versioned",
                version="1",
                name="Versioned 1",
                input_schema="audio.v1",
                output_schema="markers.v1",
                estimated_cost="light",
                run=noop,
            )
        )
        registry.register(
            TransformSpec(
                id="test.versioned",
                version="2",
                name="Versioned 2",
                input_schema="audio.v1",
                output_schema="markers.v1",
                estimated_cost="light",
                run=noop,
            )
        )

        model = TransformSpecModel(registry)
        versions = [
            model.data(model.index(row, 0), model.role_for_name("version"))
            for row in range(model.rowCount())
        ]

        self.assertEqual(versions, ["1", "2"])


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run transform picker tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_transform_picker -v
```

Expected: FAIL with `ModuleNotFoundError: No module named 'autolight.timeline.transform_model'`.

- [ ] **Step 3: Implement registry spec listing and `TransformSpecModel`**

Add this method to `TransformRegistry` in `autolight/analysis/registry.py`:

```python
    def specs(self) -> list[TransformSpec]:
        return [
            self._transforms[transform_id][version]
            for transform_id in sorted(self._transforms)
            for version in sorted(self._transforms[transform_id])
        ]
```

Create `autolight/timeline/transform_model.py`:

```python
from __future__ import annotations

from PySide6.QtCore import QAbstractListModel, QModelIndex, QObject, Qt, Slot

from autolight.analysis.registry import TransformRegistry, TransformSpec


class TransformSpecModel(QAbstractListModel):
    ROLE_NAMES = {
        Qt.ItemDataRole.UserRole + 1: b"transformId",
        Qt.ItemDataRole.UserRole + 2: b"version",
        Qt.ItemDataRole.UserRole + 3: b"name",
        Qt.ItemDataRole.UserRole + 4: b"estimatedCost",
        Qt.ItemDataRole.UserRole + 5: b"outputSchema",
    }

    def __init__(self, registry: TransformRegistry, parent: QObject | None = None):
        super().__init__(parent)
        self._specs: list[TransformSpec] = registry.specs()
        self._role_by_name = {
            value.decode("utf-8"): role for role, value in self.ROLE_NAMES.items()
        }

    def rowCount(self, parent: QModelIndex = QModelIndex()) -> int:
        return 0 if parent.isValid() else len(self._specs)

    def data(self, index: QModelIndex, role: int = Qt.ItemDataRole.DisplayRole):
        if not index.isValid() or index.row() < 0 or index.row() >= len(self._specs):
            return None
        spec = self._specs[index.row()]
        if role == Qt.ItemDataRole.DisplayRole or role == self.role_for_name("name"):
            return spec.name
        if role == self.role_for_name("transformId"):
            return spec.id
        if role == self.role_for_name("version"):
            return spec.version
        if role == self.role_for_name("estimatedCost"):
            return spec.estimated_cost
        if role == self.role_for_name("outputSchema"):
            return spec.output_schema
        return None

    def roleNames(self):
        return dict(self.ROLE_NAMES)

    def role_for_name(self, name: str) -> int:
        return self._role_by_name[name]

    @Slot(int, result=str)
    def version_at(self, row: int) -> str:
        if row < 0 or row >= len(self._specs):
            return ""
        return self._specs[row].version
```

- [ ] **Step 4: Run transform picker tests**

Run:

```bash
uv run python -m unittest tests.test_transform_picker -v
```

Expected: PASS.

- [ ] **Step 5: Commit transform spec model**

Run:

```bash
git add autolight/analysis/registry.py autolight/timeline/transform_model.py tests/test_transform_picker.py
git commit -m "Add transform spec model"
```

Expected: commit succeeds.

## Task 2: Generic Add Transform Controller Slot

**Files:**
- Modify: `autolight/app_controller.py`
- Modify: `tests/test_transform_picker.py`

- [ ] **Step 1: Add failing controller generic-transform test**

Append this test to `TransformPickerTest`:

```python
    def test_controller_add_transform_track_accepts_json_params(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        source_id = controller.trackModel.data(
            controller.trackModel.index(0, 0),
            controller.trackModel.role_for_name("trackId"),
        )

        track_id = controller.add_transform_track(
            source_id,
            "markers.fixed_interval",
            "1",
            '{"duration": 3.0, "interval": 1.0}',
        )

        self.assertNotEqual(track_id, "")
        track = next(track for track in controller._project.tracks if track.id == track_id)
        self.assertEqual(track.transform_id, "markers.fixed_interval")
        self.assertEqual(track.transform_version, "1")
        self.assertEqual(track.transform_params, {"duration": 3.0, "interval": 1.0})

    def test_controller_add_transform_track_defaults_audio_path_for_audio_transform(self):
        from autolight.app_controller import AppController

        def noop(context, params):
            return TransformResult()

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller._registry.register(
            TransformSpec(
                id="test.audio_path",
                version="1",
                name="Audio Path Transform",
                input_schema="audio.v1",
                output_schema="markers.v1",
                estimated_cost="light",
                run=noop,
            )
        )
        controller.load_demo_project()
        source_id = controller.trackModel.data(
            controller.trackModel.index(0, 0),
            controller.trackModel.role_for_name("trackId"),
        )

        track_id = controller.add_transform_track(source_id, "test.audio_path", "1", "{}")

        track = next(track for track in controller._project.tracks if track.id == track_id)
        self.assertIn("audio_path", track.transform_params)
        self.assertTrue(track.transform_params["audio_path"].endswith(".wav"))

    def test_controller_add_transform_track_resolves_audio_path_from_parent_chain(self):
        from autolight.app_controller import AppController

        def noop(context, params):
            return TransformResult()

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller._registry.register(
            TransformSpec(
                id="test.audio_path",
                version="1",
                name="Audio Path Transform",
                input_schema="audio.v1",
                output_schema="markers.v1",
                estimated_cost="light",
                run=noop,
            )
        )
        controller.load_demo_project()
        source_id = controller.trackModel.data(
            controller.trackModel.index(0, 0),
            controller.trackModel.role_for_name("trackId"),
        )
        generated_id = controller.add_transform_track(
            source_id,
            "markers.fixed_interval",
            "1",
            '{"duration": 3.0, "interval": 1.0}',
        )

        track_id = controller.add_transform_track(generated_id, "test.audio_path", "1", "{}")

        track = next(track for track in controller._project.tracks if track.id == track_id)
        self.assertIn("audio_path", track.transform_params)
        self.assertTrue(track.transform_params["audio_path"].endswith(".wav"))

    def test_controller_add_transform_track_searches_all_parent_branches_for_audio(self):
        from autolight.app_controller import AppController

        def noop(context, params):
            return TransformResult()

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller._registry.register(
            TransformSpec(
                id="test.audio_path",
                version="1",
                name="Audio Path Transform",
                input_schema="audio.v1",
                output_schema="markers.v1",
                estimated_cost="light",
                run=noop,
            )
        )
        controller.load_demo_project()
        source_id = controller.trackModel.data(
            controller.trackModel.index(0, 0),
            controller.trackModel.role_for_name("trackId"),
        )
        no_audio = Track(id="track_no_audio", type=TrackType.EDITABLE, name="No Audio")
        multi_parent = Track(
            id="track_multi_parent",
            type=TrackType.GENERATED,
            name="Multi Parent",
            input_track_ids=[no_audio.id, source_id],
        )
        controller._project.tracks.extend([no_audio, multi_parent])

        track_id = controller.add_transform_track(multi_parent.id, "test.audio_path", "1", "{}")

        track = next(track for track in controller._project.tracks if track.id == track_id)
        self.assertIn("audio_path", track.transform_params)
        self.assertTrue(track.transform_params["audio_path"].endswith(".wav"))

    def test_controller_add_transform_track_rejects_audio_transform_without_source_audio(self):
        from autolight.app_controller import AppController

        def noop(context, params):
            return TransformResult()

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller._registry.register(
            TransformSpec(
                id="test.audio_path",
                version="1",
                name="Audio Path Transform",
                input_schema="audio.v1",
                output_schema="markers.v1",
                estimated_cost="light",
                run=noop,
            )
        )
        controller.load_demo_project()
        no_audio = Track(id="track_no_audio", type=TrackType.EDITABLE, name="No Audio")
        controller._project.tracks.append(no_audio)

        track_id = controller.add_transform_track(no_audio.id, "test.audio_path", "1", "{}")

        self.assertEqual(track_id, "")
        self.assertIn("source audio track", controller.lastError)
```

- [ ] **Step 2: Run generic-transform test and verify failure**

Run:

```bash
uv run python -m unittest tests.test_transform_picker.TransformPickerTest.test_controller_add_transform_track_accepts_json_params -v
```

Expected: FAIL because `add_transform_track` is missing.

- [ ] **Step 3: Implement controller generic transform support**

Add imports to `autolight/app_controller.py`:

```python
import json

from autolight.cache.keys import track_dependency_hash
from autolight.project.store import find_track
from autolight.timeline.transform_model import TransformSpecModel
```

In `__init__`, after registering transforms:

```python
        self._transform_model = TransformSpecModel(self._registry, parent=self)
```

Add property:

```python
    @Property(QObject, constant=True)
    def transformModel(self):
        return self._transform_model
```

Add slot:

```python
    @Slot(str, str, str, str, result=str)
    def add_transform_track(self, parent_track_id: str, transform_id: str, version: str, params_json: str) -> str:
        try:
            params = json.loads(params_json or "{}")
            if not isinstance(params, dict):
                raise ValueError("transform params must be a JSON object")
            parent = find_track(self._project, parent_track_id)
            if parent is None:
                raise ValueError(f"parent track not found: {parent_track_id}")
            spec = self._registry.get(transform_id, version=version)
            params = self._params_with_parent_defaults(parent, spec, params)
            dependency_hash = track_dependency_hash(parent.cache_refs, spec.id, spec.version, params)
            track = add_generated_track(
                self._project,
                parent_track_id=parent.id,
                name=spec.name,
                transform_id=spec.id,
                transform_params=params,
                transform_version=spec.version,
                output_schema=spec.output_schema,
                dependency_hash=dependency_hash,
            )
            self._track_model.set_project(self._project)
            self._set_selected_track_id(track.id)
            self._set_last_error("")
            return track.id
        except Exception as exc:
            self._set_last_error(str(exc))
            return ""
```

Keep the `ValueError` path intentional: an `audio.v1` transform with no reachable source audio should fail at add time and set `lastError` instead of creating a child track that later fails at run time without `audio_path`.

Add this helper near the other private controller helpers:

```python
    def _params_with_parent_defaults(self, parent, spec, params: dict) -> dict:
        enriched = dict(params)
        if spec.input_schema == "audio.v1" and "audio_path" not in enriched:
            audio_path = self._source_audio_path_for_track(parent)
            if not audio_path:
                raise ValueError("audio transform requires a source audio track")
            enriched["audio_path"] = audio_path
        return enriched

    def _source_audio_path_for_track(self, track) -> str:
        seen_track_ids = set()
        pending = [track]
        while pending:
            current = pending.pop(0)
            if current is None or current.id in seen_track_ids:
                continue
            seen_track_ids.add(current.id)
            asset_id = current.provenance.get("asset_id")
            asset = next((item for item in self._project.audio_assets if item.id == asset_id), None)
            if asset is not None:
                return asset.path
            next_track_ids = list(current.input_track_ids)
            source_track_id = current.provenance.get("source_track_id", "")
            if source_track_id:
                next_track_ids.append(source_track_id)
            for next_track_id in next_track_ids:
                candidate = find_track(self._project, next_track_id)
                if candidate is not None:
                    pending.append(candidate)
        return ""
```

- [ ] **Step 4: Run transform picker tests**

Run:

```bash
uv run python -m unittest tests.test_transform_picker -v
```

Expected: PASS.

- [ ] **Step 5: Commit generic transform controller**

Run:

```bash
git add autolight/app_controller.py tests/test_transform_picker.py
git commit -m "Add generic transform track controller action"
```

Expected: commit succeeds.

## Task 3: QML Transform Picker

**Files:**
- Modify: `UI/Main.qml`
- Modify: `tests/test_transform_picker.py`

- [ ] **Step 1: Add failing QML picker test**

Add this test:

```python
    def test_qml_uses_transform_model_and_generic_add_action(self):
        from pathlib import Path

        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")
        self.assertIn("model: appController.transformModel", qml)
        self.assertIn("textRole: \"name\"", qml)
        self.assertIn("appController.add_transform_track(", qml)
        self.assertIn("appController.transformModel.version_at(transformPicker.currentIndex)", qml)
        self.assertIn("transformParamsField.text", qml)
```

- [ ] **Step 2: Run QML picker test and verify failure**

Run:

```bash
uv run python -m unittest tests.test_transform_picker.TransformPickerTest.test_qml_uses_transform_model_and_generic_add_action -v
```

Expected: FAIL because QML still uses hard-coded transform actions.

- [ ] **Step 3: Add QML transform picker controls**

Add these controls to the toolbar near `Add Markers`:

```qml
                ComboBox {
                    id: transformPicker
                    model: appController.transformModel
                    textRole: "name"
                    valueRole: "transformId"
                    Layout.preferredWidth: 190
                }

                TextField {
                    id: transformParamsField
                    text: "{\"duration\": 8.0, \"interval\": 0.5}"
                    placeholderText: "JSON params"
                    Layout.preferredWidth: 210
                }

                Button {
                    text: "Add Transform"
                    enabled: appController.selectedTrackId.length > 0 && transformPicker.currentIndex >= 0
                    onClicked: appController.add_transform_track(
                        appController.selectedTrackId,
                        transformPicker.currentValue,
                        appController.transformModel.version_at(transformPicker.currentIndex),
                        transformParamsField.text
                    )
                }
```

- [ ] **Step 4: Run picker tests and smoke**

Run:

```bash
uv run python -m unittest tests.test_transform_picker -v
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: tests pass and smoke exits 0.

- [ ] **Step 5: Commit transform picker UI**

Run:

```bash
git add UI/Main.qml tests/test_transform_picker.py
git commit -m "Add QML transform picker"
```

Expected: commit succeeds.

## Final Verification

- [ ] **Step 1: Run all tests and smoke**

Run:

```bash
uv run python -m unittest discover -s tests -v
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: tests pass and smoke exits 0.

- [ ] **Step 2: Check diff**

Run:

```bash
git diff --check
git status --short --branch
```

Expected: no whitespace errors and only intentional changes remain.
