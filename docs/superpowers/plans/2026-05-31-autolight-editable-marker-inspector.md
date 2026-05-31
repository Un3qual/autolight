# Autolight Editable Marker Inspector Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users select editable marker tracks, inspect marker metadata, and add/delete simple cue markers without mutating generated tracks.

**Architecture:** Add focused project-store helpers for editable marker mutation and expose them through controller slots. QML shows a compact inspector for the selected track; generated tracks remain read-only by controller validation.

**Tech Stack:** Python 3.14, PySide6/QML, `unittest`, existing `Marker`, `TrackType`, `ProjectStore`, and `TimelineTrackModel`.

**Prerequisite:** Complete `2026-05-31-autolight-project-workflow.md` first. This plan reuses `selectedTrackId`, `_selected_track_id`, `_set_last_error`, and selected-row QML state from that workflow.

---

## File Structure

- Modify `autolight/project/store.py`: add `add_editable_marker` and `delete_editable_marker`.
- Modify `autolight/project/__init__.py`: export marker edit helpers.
- Modify `autolight/app_controller.py`: expose selected track marker summary and marker mutation slots.
- Modify `UI/Main.qml`: add inspector panel controls for editable markers.
- Create `tests/test_editable_marker_inspector.py`: project helper, controller, and QML wiring coverage.

## Task 1: Editable Marker Mutation Helpers

**Files:**
- Modify: `autolight/project/store.py`
- Modify: `autolight/project/__init__.py`
- Create: `tests/test_editable_marker_inspector.py`

- [ ] **Step 1: Write failing marker mutation tests**

Create `tests/test_editable_marker_inspector.py`:

```python
import math
import tempfile
import unittest
import wave
from pathlib import Path

from autolight.project.models import Marker, TrackType
from autolight.project.store import (
    add_editable_marker,
    create_editable_track_from_markers,
    delete_editable_marker,
    import_audio_asset,
    new_project,
)


def write_wav(path: Path) -> None:
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(8000)
        handle.writeframes(b"\0\0" * 8000)


class EditableMarkerInspectorTest(unittest.TestCase):
    def test_add_editable_marker_rejects_generated_track(self):
        project = new_project("Demo")
        generated = self._generated_track(project)

        with self.assertRaisesRegex(ValueError, "editable track"):
            add_editable_marker(project, generated.id, 1.0, "Cue")

    def test_add_and_delete_editable_marker(self):
        project = new_project("Demo")
        generated = self._generated_track(project)
        project.markers.append(Marker(id="marker_source", track_id=generated.id, timestamp=0.5))
        editable = create_editable_track_from_markers(project, generated.id, "Editable", ["marker_source"])

        marker = add_editable_marker(project, editable.id, 1.25, "Cue")
        deleted = delete_editable_marker(project, editable.id, marker.id)

        self.assertTrue(deleted)
        self.assertNotIn(marker.id, [item.id for item in project.markers if item.track_id == editable.id])

    def test_add_editable_marker_rejects_non_finite_timestamp(self):
        project = new_project("Demo")
        generated = self._generated_track(project)
        project.markers.append(Marker(id="marker_source", track_id=generated.id, timestamp=0.5))
        editable = create_editable_track_from_markers(project, generated.id, "Editable", ["marker_source"])

        with self.assertRaisesRegex(ValueError, "finite"):
            add_editable_marker(project, editable.id, math.nan, "Cue")

        with self.assertRaisesRegex(ValueError, "finite"):
            add_editable_marker(project, editable.id, math.inf, "Cue")

    def _generated_track(self, project):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            source = import_audio_asset(project, audio_path)
        from autolight.project.store import add_generated_track

        return add_generated_track(project, source.id, "Generated", "markers.fixed_interval", {}, "1", "markers.v1", "hash")


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run marker mutation tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_editable_marker_inspector -v
```

Expected: FAIL because `add_editable_marker` and `delete_editable_marker` are missing.

- [ ] **Step 3: Implement marker mutation helpers**

Add this code to `autolight/project/store.py`:

```python
import math


def add_editable_marker(project: ProjectDocument, track_id: str, timestamp: float, label: str) -> Marker:
    track = find_track(project, track_id)
    if track is None:
        raise ValueError(f"track not found: {track_id}")
    if track.type != TrackType.EDITABLE:
        raise ValueError("markers can only be added to an editable track")
    timestamp_value = float(timestamp)
    if not math.isfinite(timestamp_value):
        raise ValueError("marker timestamp must be finite")
    marker = Marker(
        id=new_id("marker"),
        track_id=track_id,
        timestamp=timestamp_value,
        label=str(label),
        category="cue",
        metadata={"created_by": "user"},
    )
    project.markers.append(marker)
    return marker


def delete_editable_marker(project: ProjectDocument, track_id: str, marker_id: str) -> bool:
    track = find_track(project, track_id)
    if track is None:
        raise ValueError(f"track not found: {track_id}")
    if track.type != TrackType.EDITABLE:
        raise ValueError("markers can only be deleted from an editable track")
    before = len(project.markers)
    project.markers[:] = [
        marker for marker in project.markers if not (marker.track_id == track_id and marker.id == marker_id)
    ]
    return len(project.markers) != before
```

Export both helpers from `autolight/project/__init__.py`.

- [ ] **Step 4: Run marker mutation tests**

Run:

```bash
uv run python -m unittest tests.test_editable_marker_inspector -v
```

Expected: PASS.

- [ ] **Step 5: Commit marker mutation helpers**

Run:

```bash
git add autolight/project/store.py autolight/project/__init__.py tests/test_editable_marker_inspector.py
git commit -m "Add editable marker mutation helpers"
```

Expected: commit succeeds.

## Task 2: Controller Inspector Slots

**Files:**
- Modify: `autolight/app_controller.py`
- Modify: `tests/test_editable_marker_inspector.py`

- [ ] **Step 1: Add failing controller inspector tests**

Add this test:

```python
    def _track_id_for_type(self, controller, track_type: str) -> str:
        model = controller.trackModel
        type_role = model.role_for_name("trackType")
        id_role = model.role_for_name("trackId")
        for row in range(model.rowCount()):
            index = model.index(row, 0)
            if model.data(index, type_role) == track_type:
                return model.data(index, id_role)
        raise AssertionError(f"track type not found: {track_type}")

    def test_controller_adds_marker_to_selected_editable_track(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)

        marker_id = controller.add_marker_to_selected_track(1.5, "Blackout")

        self.assertNotEqual(marker_id, "")
        self.assertEqual(controller.lastError, "")
        self.assertTrue(any(marker.id == marker_id for marker in controller._project.markers))

    def test_controller_rejects_non_finite_marker_timestamp(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)

        marker_id = controller.add_marker_to_selected_track(math.nan, "Broken")

        self.assertEqual(marker_id, "")
        self.assertIn("finite", controller.lastError)

    def test_controller_deletes_marker_from_selected_editable_track(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        marker_id = controller.add_marker_to_selected_track(1.5, "Blackout")

        self.assertTrue(controller.delete_marker_from_selected_track(marker_id))

        self.assertFalse(any(marker.id == marker_id for marker in controller._project.markers))
        self.assertEqual(controller.lastError, "")
```

- [ ] **Step 2: Run controller inspector test and verify failure**

Run:

```bash
uv run python -m unittest tests.test_editable_marker_inspector.EditableMarkerInspectorTest.test_controller_adds_marker_to_selected_editable_track tests.test_editable_marker_inspector.EditableMarkerInspectorTest.test_controller_deletes_marker_from_selected_editable_track -v
```

Expected: FAIL because `add_marker_to_selected_track` is missing.

- [ ] **Step 3: Implement controller marker slots**

Extend imports:

```python
from autolight.project.store import add_editable_marker, delete_editable_marker
```

Add signal, property, and helper:

```python
    selectedTrackMarkersChanged = Signal()

    @Property(list, notify=selectedTrackMarkersChanged)
    def selectedTrackMarkers(self) -> list[dict]:
        return self._marker_summary_for_track(self._selected_track_id)

    def _marker_summary_for_track(self, track_id: str) -> list[dict]:
        return [
            {
                "id": marker.id,
                "timestamp": marker.timestamp,
                "label": marker.label,
                "category": marker.category,
            }
            for marker in sorted(
                (marker for marker in self._project.markers if marker.track_id == track_id),
                key=lambda marker: (marker.timestamp, marker.id),
            )
        ]
```

Update `_set_selected_track_id` so it emits `selectedTrackMarkersChanged` whenever the selected track changes.

Add slots:

```python
    @Slot(float, str, result=str)
    def add_marker_to_selected_track(self, timestamp: float, label: str) -> str:
        try:
            marker = add_editable_marker(self._project, self._selected_track_id, timestamp, label)
            self._track_model.refresh_track(self._selected_track_id)
            self.selectedTrackMarkersChanged.emit()
            self._set_last_error("")
            return marker.id
        except Exception as exc:
            self._set_last_error(str(exc))
            return ""

    @Slot(str, result=bool)
    def delete_marker_from_selected_track(self, marker_id: str) -> bool:
        try:
            deleted = delete_editable_marker(self._project, self._selected_track_id, marker_id)
            self._track_model.refresh_track(self._selected_track_id)
            self.selectedTrackMarkersChanged.emit()
            self._set_last_error("")
            return deleted
        except Exception as exc:
            self._set_last_error(str(exc))
            return False
```

- [ ] **Step 4: Run editable marker tests**

Run:

```bash
uv run python -m unittest tests.test_editable_marker_inspector -v
```

Expected: PASS.

- [ ] **Step 5: Commit controller inspector slots**

Run:

```bash
git add autolight/app_controller.py tests/test_editable_marker_inspector.py
git commit -m "Add editable marker controller slots"
```

Expected: commit succeeds.

## Task 3: QML Inspector Controls

**Files:**
- Modify: `UI/Main.qml`
- Modify: `tests/test_editable_marker_inspector.py`

- [ ] **Step 1: Add failing QML inspector test**

Add this test:

```python
    def test_qml_exposes_editable_marker_inspector(self):
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")

        self.assertIn("id: inspectorPanel", qml)
        self.assertIn("markerTimestampField", qml)
        self.assertIn("markerLabelField", qml)
        self.assertIn("appController.selectedTrackMarkers", qml)
        self.assertIn("inspectorPanel.selectedMarkerId", qml)
        self.assertIn("appController.add_marker_to_selected_track", qml)
        self.assertIn("appController.delete_marker_from_selected_track(inspectorPanel.selectedMarkerId)", qml)
        self.assertEqual(qml.count("ListView {"), 1)
```

- [ ] **Step 2: Run QML inspector test and verify failure**

Run:

```bash
uv run python -m unittest tests.test_editable_marker_inspector.EditableMarkerInspectorTest.test_qml_exposes_editable_marker_inspector -v
```

Expected: FAIL because the inspector controls are not present.

- [ ] **Step 3: Add QML inspector panel**

Use `ScrollView` and `Repeater` for the marker list rather than adding a second `ListView`, so the existing timeline shell test that enforces one row-oriented timeline `ListView` remains meaningful.

Add a right-side `Rectangle` with this core content:

```qml
            Rectangle {
                id: inspectorPanel
                Layout.preferredWidth: 260
                Layout.fillHeight: true
                color: "#1c1f26"
                border.color: "#2f333d"
                property string selectedMarkerId: ""

                Connections {
                    target: appController
                    function onSelectedTrackIdChanged() {
                        inspectorPanel.selectedMarkerId = ""
                    }
                }

                Column {
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 8

                    Label {
                        text: "Inspector"
                        color: "#f4f4f5"
                        font.bold: true
                    }

                    TextField {
                        id: markerTimestampField
                        placeholderText: "Timestamp"
                        text: "0.0"
                    }

                    TextField {
                        id: markerLabelField
                        placeholderText: "Label"
                        text: "Cue"
                    }

                    ScrollView {
                        id: markerScroll
                        width: parent.width
                        height: 120
                        clip: true

                        Column {
                            id: markerList
                            width: markerScroll.availableWidth
                            spacing: 2

                            Repeater {
                                model: appController.selectedTrackMarkers
                                delegate: Rectangle {
                                    required property var modelData
                                    width: markerList.width
                                    height: 28
                                    color: inspectorPanel.selectedMarkerId === modelData.id ? "#2f4366" : "transparent"

                                    Text {
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: Number(modelData.timestamp).toFixed(2) + "  " + modelData.label
                                        color: "#f4f4f5"
                                        elide: Text.ElideRight
                                        width: parent.width
                                    }

                                    MouseArea {
                                        anchors.fill: parent
                                        onClicked: inspectorPanel.selectedMarkerId = modelData.id
                                    }
                                }
                            }
                        }
                    }

                    Button {
                        text: "Add Cue"
                        enabled: appController.selectedTrackId.length > 0
                        onClicked: appController.add_marker_to_selected_track(
                            Number(markerTimestampField.text),
                            markerLabelField.text
                        )
                    }

                    Button {
                        text: "Delete Cue"
                        enabled: inspectorPanel.selectedMarkerId.length > 0
                        onClicked: {
                            if (appController.delete_marker_from_selected_track(inspectorPanel.selectedMarkerId)) {
                                inspectorPanel.selectedMarkerId = ""
                            }
                        }
                    }
                }
            }
```

Wrap the existing timeline `ListView` and the new inspector in a `RowLayout`; keep the existing `ListView` as `Layout.fillWidth: true` and put the inspector `Rectangle` beside it.

- [ ] **Step 4: Run inspector tests and smoke**

Run:

```bash
uv run python -m unittest tests.test_editable_marker_inspector -v
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: tests pass and smoke exits 0.

- [ ] **Step 5: Commit inspector UI**

Run:

```bash
git add UI/Main.qml tests/test_editable_marker_inspector.py
git commit -m "Add editable marker inspector controls"
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
