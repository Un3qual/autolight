# Autolight Stem Artifact Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the existing expensive stem stand-in transform visible and recoverable as a first-class artifact workflow.

**Architecture:** Treat `stems.vocals_stand_in` as a heavy generated track that produces cache artifacts rather than markers. Add artifact summary roles and UI status so users can run, cancel, inspect, and rerun the stem job without needing real source separation yet.

**Tech Stack:** Python 3.14, PySide6/QML, `unittest`, existing `stems.vocals_stand_in`, `LocalJobQueue`, `CacheStore`, and `TimelineTrackModel`.

**Prerequisite:** Complete `2026-05-31-autolight-project-workflow.md` first. This plan reuses `selectedTrackId`, `_set_selected_track_id`, and the selected-track QML wiring introduced there.

---

## File Structure

- Modify `autolight/timeline/model.py`: add `cacheRefCount` and `artifactKinds` roles.
- Modify `autolight/app_controller.py`: expose `add_vocals_stem_track`.
- Modify `UI/Main.qml`: add "Add Vocals Stem" action and artifact status text.
- Create `tests/test_stem_artifact_workflow.py`: controller, model, cache, and QML wiring tests.

## Task 1: Artifact Roles In Timeline Model

**Files:**
- Modify: `autolight/timeline/model.py`
- Create: `tests/test_stem_artifact_workflow.py`

- [ ] **Step 1: Add failing artifact role tests**

Create `tests/test_stem_artifact_workflow.py`:

```python
import unittest

from PySide6.QtCore import QCoreApplication

from autolight.project.models import CacheEntry, ProjectDocument, Track, TrackType
from autolight.timeline.model import TimelineTrackModel


class StemArtifactWorkflowTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication([])

    def test_timeline_model_exposes_artifact_summary_roles(self):
        project = ProjectDocument(id="project_1", name="Demo")
        project.tracks.append(
            Track(
                id="track_stem",
                type=TrackType.GENERATED,
                name="Vocals",
                cache_refs=["cache_1"],
            )
        )
        project.cache_entries.append(
            CacheEntry(
                id="cache_1",
                dependency_hash="dep",
                artifact_kind="stem",
                path="stem/cache_1.json",
                created_at="",
                transform_version="1",
            )
        )
        model = TimelineTrackModel()
        model.set_project(project)
        index = model.index(0, 0)

        self.assertEqual(model.data(index, model.role_for_name("cacheRefCount")), 1)
        self.assertEqual(model.data(index, model.role_for_name("artifactKinds")), "stem")


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run artifact role tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_stem_artifact_workflow -v
```

Expected: FAIL with `KeyError: 'cacheRefCount'`.

- [ ] **Step 3: Add artifact summary roles**

Extend `ROLE_NAMES` in `autolight/timeline/model.py`:

```python
        Qt.ItemDataRole.UserRole + 12: b"cacheRefCount",
        Qt.ItemDataRole.UserRole + 13: b"artifactKinds",
```

Add role branches:

```python
        if role == self.role_for_name("cacheRefCount"):
            return len(track.cache_refs)
        if role == self.role_for_name("artifactKinds"):
            return ", ".join(self._artifact_kinds_for_track(track.cache_refs))
```

Add helper:

```python
    def _artifact_kinds_for_track(self, cache_refs: list[str]) -> list[str]:
        if self._project is None:
            return []
        entries = {entry.id: entry for entry in self._project.cache_entries}
        return [
            entries[cache_ref].artifact_kind
            for cache_ref in cache_refs
            if cache_ref in entries
        ]
```

- [ ] **Step 4: Run stem artifact tests**

Run:

```bash
uv run python -m unittest tests.test_stem_artifact_workflow tests.test_timeline_model -v
```

Expected: PASS.

- [ ] **Step 5: Commit artifact roles**

Run:

```bash
git add autolight/timeline/model.py tests/test_stem_artifact_workflow.py
git commit -m "Expose artifact summaries in timeline model"
```

Expected: commit succeeds.

## Task 2: Controller Stem Track Action

**Files:**
- Modify: `autolight/app_controller.py`
- Modify: `tests/test_stem_artifact_workflow.py`

- [ ] **Step 1: Add failing controller stem test**

Add this test:

```python
    def test_controller_adds_vocals_stem_track(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        source_id = controller.trackModel.data(
            controller.trackModel.index(0, 0),
            controller.trackModel.role_for_name("trackId"),
        )

        stem_id = controller.add_vocals_stem_track(source_id)

        self.assertNotEqual(stem_id, "")
        stem = next(track for track in controller._project.tracks if track.id == stem_id)
        self.assertEqual(stem.transform_id, "stems.vocals_stand_in")
        self.assertEqual(stem.output_schema, "artifact.stem.v1")
```

- [ ] **Step 2: Run controller stem test and verify failure**

Run:

```bash
uv run python -m unittest tests.test_stem_artifact_workflow.StemArtifactWorkflowTest.test_controller_adds_vocals_stem_track -v
```

Expected: FAIL because `add_vocals_stem_track` is missing.

- [ ] **Step 3: Implement stem track action**

Add imports to `autolight/app_controller.py`:

```python
from autolight.cache.keys import track_dependency_hash
from autolight.project.store import find_track
```

Add slot to `autolight/app_controller.py`:

```python
    @Slot(str, result=str)
    def add_vocals_stem_track(self, parent_track_id: str) -> str:
        try:
            parent = find_track(self._project, parent_track_id)
            if parent is None:
                raise ValueError(f"parent track not found: {parent_track_id}")
            transform_id = "stems.vocals_stand_in"
            transform_version = "1"
            params = {"label": "vocals"}
            dependency_hash = track_dependency_hash(parent.cache_refs, transform_id, transform_version, params)
            track = add_generated_track(
                self._project,
                parent_track_id=parent.id,
                name="Vocals Stem",
                transform_id=transform_id,
                transform_params=params,
                transform_version=transform_version,
                output_schema="artifact.stem.v1",
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

- [ ] **Step 4: Run stem workflow tests**

Run:

```bash
uv run python -m unittest tests.test_stem_artifact_workflow -v
```

Expected: PASS.

- [ ] **Step 5: Commit stem controller action**

Run:

```bash
git add autolight/app_controller.py tests/test_stem_artifact_workflow.py
git commit -m "Add vocals stem controller action"
```

Expected: commit succeeds.

## Task 3: Stem Workflow UI

**Files:**
- Modify: `UI/Main.qml`
- Modify: `tests/test_stem_artifact_workflow.py`

- [ ] **Step 1: Add failing QML stem workflow test**

Add this test:

```python
    def test_qml_exposes_stem_workflow(self):
        from pathlib import Path

        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")
        self.assertIn("appController.add_vocals_stem_track(appController.selectedTrackId)", qml)
        self.assertIn("artifactKinds", qml)
        self.assertIn("cacheRefCount", qml)
```

- [ ] **Step 2: Run QML stem test and verify failure**

Run:

```bash
uv run python -m unittest tests.test_stem_artifact_workflow.StemArtifactWorkflowTest.test_qml_exposes_stem_workflow -v
```

Expected: FAIL because the QML does not expose stem actions or artifact roles.

- [ ] **Step 3: Add stem UI action and artifact status**

Add toolbar button:

```qml
                Button {
                    text: "Add Vocals Stem"
                    enabled: appController.selectedTrackId.length > 0
                    onClicked: appController.add_vocals_stem_track(appController.selectedTrackId)
                }
```

Add artifact status text below the existing track status text:

```qml
                        Text {
                            text: cacheRefCount > 0 ? artifactKinds + " artifact" : ""
                            color: "#93c5fd"
                            font.pixelSize: 12
                            elide: Text.ElideRight
                            width: parent.width
                            visible: cacheRefCount > 0
                        }
```

- [ ] **Step 4: Run stem tests and smoke**

Run:

```bash
uv run python -m unittest tests.test_stem_artifact_workflow -v
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: tests pass and smoke exits 0.

- [ ] **Step 5: Commit stem UI workflow**

Run:

```bash
git add UI/Main.qml tests/test_stem_artifact_workflow.py
git commit -m "Expose stem artifact workflow in QML"
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
