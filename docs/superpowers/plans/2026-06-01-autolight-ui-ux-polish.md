# Autolight UI UX Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Polish editable marker workflows, waveform/playback inspection, persisted timeline context, and the overall QML shell so Autolight feels usable for repeated cue editing.

**Architecture:** Keep project data and validation in `autolight/project/store.py`, controller state and slots in `autolight/app_controller.py`, QML-friendly display roles in `autolight/timeline/model.py`, and visual layout in `UI/Main.qml`. Store marker colors in `Marker.metadata["color"]` as a palette key, expose resolved display colors through controller/model summaries, save timeline viewport state in existing `ProjectDocument.ui_state`, and add a screenshot/pixel-check harness for visual QA.

**Tech Stack:** Python 3.14, PySide6/QML, PySide6 `QtMultimedia`, `unittest`, `QImage` screenshot inspection, existing `AppController`, `TimelineTrackModel`, `ProjectStore`, and `PlaybackTransport`.

---

## File Structure

- Modify `autolight/project/store.py`: add marker color palette helpers, single-marker update helper, and bulk editable marker update helper.
- Modify `autolight/project/__init__.py`: export new marker editing helpers from the existing package export list.
- Modify `autolight/timeline/model.py`: include resolved marker display color in `markerSpans`.
- Modify `autolight/app_controller.py`: expose selected marker state, marker update slots, playback nudge/volume support, screenshot-friendly demo state, and timeline UI-state save/restore.
- Modify `autolight/playback/transport.py`: expose volume as a QML property and keep `set_volume()` observable.
- Modify `UI/Main.qml`: add marker label/color editing, bulk marker actions, improved waveform/playback controls, click-to-seek, viewport restoration bindings, and layout polish.
- Modify `main.py`: add deterministic `--screenshot <path>` capture mode for offscreen visual QA.
- Create `scripts/check_qml_screenshot.py`: inspect screenshot dimensions and expected visual color regions with `QImage`.
- Modify `README.md`: document marker color/bulk editing, visual QA commands, and persisted timeline context.
- Modify `tests/test_editable_marker_inspector.py`: cover marker label/color updates, bulk edits, controller marker selection, and QML wiring.
- Modify `tests/test_timeline_model.py`: cover marker display color roles.
- Modify `tests/test_app_controller.py`: cover playback UX slots, UI-state persistence, QML wiring, and screenshot CLI branch.
- Modify `tests/test_playback_transport.py`: cover observable volume state.
- Modify `tests/test_waveform_summary.py`: keep waveform QML assertions aligned with the new rendering.

## Task 1: Marker Label, Color, And Bulk Edit Store Helpers

**Files:**
- Modify: `autolight/project/store.py`
- Modify: `autolight/project/__init__.py`
- Modify: `tests/test_editable_marker_inspector.py`

- [x] **Step 1: Add failing store tests for marker polish helpers**

Add these imports to `tests/test_editable_marker_inspector.py`:

```python
from autolight.project.store import (
    MARKER_COLOR_PALETTE,
    bulk_update_editable_markers,
    marker_display_color,
    update_editable_marker,
)
```

Add these tests to `EditableMarkerInspectorTest`:

```python
    def test_update_editable_marker_sets_label_category_color_and_timestamp(self):
        project = new_project("Demo")
        editable = self._editable_track(project)
        marker = add_editable_marker(project, editable.id, 1.25, "Cue")

        updated = update_editable_marker(
            project,
            editable.id,
            marker.id,
            timestamp=2.5,
            label="Blackout",
            category="lighting",
            color="amber",
        )

        self.assertIs(updated, marker)
        self.assertEqual(marker.timestamp, 2.5)
        self.assertEqual(marker.label, "Blackout")
        self.assertEqual(marker.category, "lighting")
        self.assertEqual(marker.metadata["color"], "amber")
        self.assertEqual(marker_display_color(marker), MARKER_COLOR_PALETTE["amber"])

    def test_update_editable_marker_rejects_generated_track_and_invalid_color(self):
        project = new_project("Demo")
        generated = self._generated_track(project)
        project.markers.append(Marker(id="marker_source", track_id=generated.id, timestamp=0.5))

        with self.assertRaisesRegex(ValueError, "editable track"):
            update_editable_marker(
                project,
                generated.id,
                "marker_source",
                timestamp=1.0,
                label="Cue",
                category="cue",
                color="cyan",
            )

        editable = create_editable_track_from_markers(project, generated.id, "Editable", ["marker_source"])
        marker = [item for item in project.markers if item.track_id == editable.id][0]

        with self.assertRaisesRegex(ValueError, "marker color"):
            update_editable_marker(
                project,
                editable.id,
                marker.id,
                timestamp=1.0,
                label="Cue",
                category="cue",
                color="not-a-color",
            )

    def test_update_editable_marker_marks_downstream_generated_tracks_stale(self):
        project = new_project("Demo")
        editable = self._editable_track(project)
        marker = add_editable_marker(project, editable.id, 1.25, "Cue")
        downstream = add_generated_track(
            project,
            editable.id,
            "Generated From Editable",
            "markers.fixed_interval",
            {},
            "1",
            "markers.v1",
            "dep",
        )
        downstream.result_state = ResultState.COMPLETE

        update_editable_marker(
            project,
            editable.id,
            marker.id,
            timestamp=1.5,
            label="Look",
            category="lighting",
            color="violet",
        )

        self.assertEqual(downstream.result_state, ResultState.STALE)

    def test_bulk_update_editable_markers_updates_named_markers(self):
        project = new_project("Demo")
        editable = self._editable_track(project)
        first = add_editable_marker(project, editable.id, 1.0, "A")
        second = add_editable_marker(project, editable.id, 2.0, "B")
        third = add_editable_marker(project, editable.id, 3.0, "C")

        updated_count = bulk_update_editable_markers(
            project,
            editable.id,
            [first.id, third.id],
            label="Hit",
            category="accent",
            color="rose",
        )

        self.assertEqual(updated_count, 2)
        self.assertEqual(first.label, "Hit")
        self.assertEqual(first.category, "accent")
        self.assertEqual(first.metadata["color"], "rose")
        self.assertEqual(second.label, "B")
        self.assertNotIn("color", second.metadata)
        self.assertEqual(third.label, "Hit")

    def test_bulk_update_with_empty_marker_ids_updates_all_markers_on_track(self):
        project = new_project("Demo")
        editable = self._editable_track(project)
        first = add_editable_marker(project, editable.id, 1.0, "A")
        second = add_editable_marker(project, editable.id, 2.0, "B")

        updated_count = bulk_update_editable_markers(
            project,
            editable.id,
            [],
            label="Scene",
            category="scene",
            color="blue",
        )

        self.assertEqual(updated_count, 2)
        self.assertEqual([first.label, second.label], ["Scene", "Scene"])
        self.assertEqual([first.metadata["color"], second.metadata["color"]], ["blue", "blue"])
```

- [x] **Step 2: Run store tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_editable_marker_inspector.EditableMarkerInspectorTest.test_update_editable_marker_sets_label_category_color_and_timestamp tests.test_editable_marker_inspector.EditableMarkerInspectorTest.test_update_editable_marker_rejects_generated_track_and_invalid_color tests.test_editable_marker_inspector.EditableMarkerInspectorTest.test_update_editable_marker_marks_downstream_generated_tracks_stale tests.test_editable_marker_inspector.EditableMarkerInspectorTest.test_bulk_update_editable_markers_updates_named_markers tests.test_editable_marker_inspector.EditableMarkerInspectorTest.test_bulk_update_with_empty_marker_ids_updates_all_markers_on_track -v
```

Expected: FAIL because `update_editable_marker`, `bulk_update_editable_markers`, `marker_display_color`, and `MARKER_COLOR_PALETTE` do not exist.

- [x] **Step 3: Implement marker palette and update helpers**

In `autolight/project/store.py`, add these definitions near the existing marker helpers:

```python
MARKER_COLOR_PALETTE = {
    "cyan": "#67e8f9",
    "green": "#a7f3d0",
    "amber": "#fbbf24",
    "violet": "#c4b5fd",
    "rose": "#fda4af",
    "blue": "#93c5fd",
}
DEFAULT_MARKER_COLOR = "cyan"


def marker_display_color(marker: Marker) -> str:
    color = marker.metadata.get("color", "")
    if isinstance(color, str) and color in MARKER_COLOR_PALETTE:
        return MARKER_COLOR_PALETTE[color]
    return MARKER_COLOR_PALETTE[DEFAULT_MARKER_COLOR]


def _editable_track_or_raise(project: ProjectDocument, track_id: str) -> Track:
    track = find_track(project, track_id)
    if track is None:
        raise ValueError(f"track not found: {track_id}")
    if track.type != TrackType.EDITABLE:
        raise ValueError("markers can only be edited on an editable track")
    return track


def _editable_marker_or_raise(project: ProjectDocument, track_id: str, marker_id: str) -> Marker:
    for marker in project.markers:
        if marker.track_id == track_id and marker.id == marker_id:
            return marker
    raise ValueError(f"marker not found on track {track_id}: {marker_id}")


def _finite_marker_timestamp(timestamp: float) -> float:
    timestamp_value = float(timestamp)
    if not math.isfinite(timestamp_value):
        raise ValueError("marker timestamp must be finite")
    return timestamp_value


def _normalized_marker_color(color: str) -> str:
    value = str(color or DEFAULT_MARKER_COLOR).strip().lower()
    if value not in MARKER_COLOR_PALETTE:
        raise ValueError(f"marker color must be one of: {', '.join(MARKER_COLOR_PALETTE)}")
    return value


def _apply_marker_fields(
    marker: Marker,
    *,
    timestamp: float | None = None,
    label: str | None = None,
    category: str | None = None,
    color: str | None = None,
) -> bool:
    changed = False
    if timestamp is not None:
        timestamp_value = _finite_marker_timestamp(timestamp)
        if marker.timestamp != timestamp_value:
            marker.timestamp = timestamp_value
            changed = True
    if label is not None:
        label_value = str(label)
        if marker.label != label_value:
            marker.label = label_value
            changed = True
    if category is not None:
        category_value = str(category or "cue")
        if marker.category != category_value:
            marker.category = category_value
            changed = True
    if color is not None:
        color_value = _normalized_marker_color(color)
        if marker.metadata.get("color") != color_value:
            marker.metadata["color"] = color_value
            changed = True
    return changed


def update_editable_marker(
    project: ProjectDocument,
    track_id: str,
    marker_id: str,
    *,
    timestamp: float,
    label: str,
    category: str,
    color: str,
) -> Marker:
    _editable_track_or_raise(project, track_id)
    marker = _editable_marker_or_raise(project, track_id, marker_id)
    changed = _apply_marker_fields(
        marker,
        timestamp=timestamp,
        label=label,
        category=category,
        color=color,
    )
    if changed:
        mark_dependents_stale(project, track_id)
    return marker


def bulk_update_editable_markers(
    project: ProjectDocument,
    track_id: str,
    marker_ids: list[str],
    *,
    label: str,
    category: str,
    color: str,
) -> int:
    _editable_track_or_raise(project, track_id)
    selected_ids = set(marker_ids)
    changed_count = 0
    for marker in project.markers:
        if marker.track_id != track_id:
            continue
        if selected_ids and marker.id not in selected_ids:
            continue
        if _apply_marker_fields(marker, label=label, category=category, color=color):
            changed_count += 1
    if changed_count:
        mark_dependents_stale(project, track_id)
    return changed_count
```

Update `add_editable_marker()` so new markers get a default color:

```python
    marker = Marker(
        id=new_id("marker"),
        track_id=track_id,
        timestamp=timestamp_value,
        label=str(label),
        category="cue",
        metadata={"created_by": "user", "color": DEFAULT_MARKER_COLOR},
    )
```

In `autolight/project/__init__.py`, add these names to the `from autolight.project.store import (...)` block:

```python
    MARKER_COLOR_PALETTE,
    bulk_update_editable_markers,
    marker_display_color,
    update_editable_marker,
```

Add the same public names to `__all__`:

```python
    "MARKER_COLOR_PALETTE",
    "bulk_update_editable_markers",
    "marker_display_color",
    "update_editable_marker",
```

- [x] **Step 4: Run focused marker helper tests**

Run:

```bash
uv run python -m unittest tests.test_editable_marker_inspector -v
```

Expected: PASS for the marker helper, controller, and existing inspector tests.

- [x] **Step 5: Commit marker helper work**

Run:

```bash
git add autolight/project/store.py autolight/project/__init__.py tests/test_editable_marker_inspector.py
git commit -m "Polish editable marker metadata helpers"
```

Expected: commit succeeds.

## Task 2: Marker Editing Controller State And Timeline Roles

**Files:**
- Modify: `autolight/app_controller.py`
- Modify: `autolight/timeline/model.py`
- Modify: `tests/test_editable_marker_inspector.py`
- Modify: `tests/test_timeline_model.py`

- [x] **Step 1: Add failing controller tests for selected markers and bulk slots**

Add these tests to `tests/test_editable_marker_inspector.py`:

```python
    def test_controller_tracks_selected_marker_ids(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        first_marker_id = controller.selectedTrackMarkers[0]["id"]
        second_marker_id = controller.selectedTrackMarkers[1]["id"]

        controller.toggle_marker_selection(first_marker_id, False)
        controller.toggle_marker_selection(second_marker_id, True)

        self.assertEqual(controller.selectedMarkerIds, [first_marker_id, second_marker_id])
        self.assertTrue(controller.selectedTrackMarkers[0]["selected"])
        self.assertTrue(controller.selectedTrackMarkers[1]["selected"])

    def test_controller_update_selected_marker_changes_marker_fields(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        marker_id = controller.selectedTrackMarkers[0]["id"]
        controller.toggle_marker_selection(marker_id, False)

        self.assertTrue(controller.update_selected_marker(1.75, "Blackout", "lighting", "amber"))

        marker = next(item for item in controller._project.markers if item.id == marker_id)
        self.assertEqual(marker.timestamp, 1.75)
        self.assertEqual(marker.label, "Blackout")
        self.assertEqual(marker.category, "lighting")
        self.assertEqual(marker.metadata["color"], "amber")
        self.assertTrue(controller.isDirty)

    def test_controller_bulk_update_selected_markers_updates_selected_or_all(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        first_marker_id = controller.selectedTrackMarkers[0]["id"]
        second_marker_id = controller.selectedTrackMarkers[1]["id"]
        controller.toggle_marker_selection(first_marker_id, False)

        self.assertEqual(controller.bulk_update_selected_markers("Scene", "scene", "violet"), 1)
        first = next(item for item in controller._project.markers if item.id == first_marker_id)
        second = next(item for item in controller._project.markers if item.id == second_marker_id)
        self.assertEqual(first.label, "Scene")
        self.assertNotEqual(second.label, "Scene")

        controller.clear_marker_selection()
        self.assertEqual(controller.bulk_update_selected_markers("All", "scene", "blue"), 2)
        self.assertEqual([item["label"] for item in controller.selectedTrackMarkers], ["All", "All"])
```

- [x] **Step 2: Add failing timeline model color role test**

In `tests/test_timeline_model.py`, add `marker_display_color` import:

```python
from autolight.project.store import marker_display_color
```

In `test_model_exposes_track_roles_for_qml`, change the expected `markerSpans` entry to include `color`:

```python
                [
                    {
                        "id": "marker_1",
                        "timestamp": 0.5,
                        "duration": 0.0,
                        "label": "",
                        "category": "",
                        "color": "#67e8f9",
                    }
                ],
```

Add a dedicated color test:

```python
    def test_marker_spans_resolve_marker_color_metadata(self):
        with tempfile.TemporaryDirectory() as tmp:
            project, _source, generated = self._project_with_generated_track(Path(tmp))
            marker = Marker(
                id="marker_amber",
                track_id=generated.id,
                timestamp=0.5,
                label="Look",
                metadata={"color": "amber"},
            )
            project.markers.append(marker)
            model = TimelineTrackModel()
            model.set_project(project)

            span = model.data(model.index(1, 0), model.role_for_name("markerSpans"))[0]

            self.assertEqual(span["color"], marker_display_color(marker))
```

- [x] **Step 3: Run marker controller and timeline tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_editable_marker_inspector tests.test_timeline_model.TimelineTrackModelTest.test_model_exposes_track_roles_for_qml tests.test_timeline_model.TimelineTrackModelTest.test_marker_spans_resolve_marker_color_metadata -v
```

Expected: FAIL because the controller marker selection API and marker color span field are missing.

- [x] **Step 4: Add controller marker selection properties and slots**

Update imports in `autolight/app_controller.py`:

```python
from autolight.project.store import (
    MARKER_COLOR_PALETTE,
    ProjectStore,
    add_editable_marker,
    add_generated_track,
    bulk_update_editable_markers,
    create_editable_track_from_markers,
    delete_editable_marker,
    find_track,
    import_audio_asset,
    marker_display_color,
    new_project,
    refresh_audio_asset_status,
    refresh_audio_track_status,
    track_dependency_inputs,
    update_editable_marker,
)
```

Add a signal and property to `AppController`:

```python
    selectedMarkerIdsChanged = Signal()

    @Property(list, notify=selectedMarkerIdsChanged)
    def selectedMarkerIds(self) -> list[str]:
        return list(self._selected_marker_ids)
```

Initialize selected markers in `__init__`:

```python
        self._selected_marker_ids: list[str] = []
```

Add slots:

```python
    @Slot(str, bool)
    def toggle_marker_selection(self, marker_id: str, additive: bool) -> None:
        marker_ids = {marker["id"] for marker in self._marker_summary_for_track(self._selected_track_id)}
        if marker_id not in marker_ids:
            self._set_last_error(f"marker not found: {marker_id}")
            return
        if additive:
            selected = list(self._selected_marker_ids)
            if marker_id in selected:
                selected.remove(marker_id)
            else:
                selected.append(marker_id)
            self._set_selected_marker_ids(selected)
        else:
            self._set_selected_marker_ids([marker_id])
        self._set_last_error("")

    @Slot()
    def clear_marker_selection(self) -> None:
        self._set_selected_marker_ids([])

    @Slot(float, str, str, str, result=bool)
    def update_selected_marker(self, timestamp: float, label: str, category: str, color: str) -> bool:
        try:
            if len(self._selected_marker_ids) != 1:
                raise ValueError("select one marker to update")
            update_editable_marker(
                self._project,
                self._selected_track_id,
                self._selected_marker_ids[0],
                timestamp=timestamp,
                label=label,
                category=category,
                color=color,
            )
            self._track_model.set_project(self._project)
            self.selectedTrackMarkersChanged.emit()
            self._notify_timeline_duration_changed()
            self._set_last_error("")
            self._set_dirty(True)
            return True
        except Exception as exc:
            self._set_last_error(str(exc))
            return False

    @Slot(str, str, str, result=int)
    def bulk_update_selected_markers(self, label: str, category: str, color: str) -> int:
        try:
            updated = bulk_update_editable_markers(
                self._project,
                self._selected_track_id,
                self._selected_marker_ids,
                label=label,
                category=category,
                color=color,
            )
            self._track_model.set_project(self._project)
            self.selectedTrackMarkersChanged.emit()
            self._set_last_error("")
            if updated:
                self._set_dirty(True)
            return updated
        except Exception as exc:
            self._set_last_error(str(exc))
            return 0
```

Add helper:

```python
    def _set_selected_marker_ids(self, marker_ids: list[str]) -> None:
        if self._selected_marker_ids == marker_ids:
            return
        self._selected_marker_ids = list(marker_ids)
        self.selectedMarkerIdsChanged.emit()
        self.selectedTrackMarkersChanged.emit()
```

Reset marker selection in `_set_selected_track_id`:

```python
        self._set_selected_marker_ids([])
```

Update `_marker_summary_for_track`:

```python
    def _marker_summary_for_track(self, track_id: str) -> list[dict]:
        selected_ids = set(self._selected_marker_ids)
        return [
            {
                "id": marker.id,
                "timestamp": marker.timestamp,
                "label": marker.label,
                "category": marker.category,
                "color": marker_display_color(marker),
                "colorKey": marker.metadata.get("color", "cyan"),
                "selected": marker.id in selected_ids,
            }
            for marker in sorted(
                (marker for marker in self._project.markers if marker.track_id == track_id),
                key=lambda marker: (marker.timestamp, marker.id),
            )
        ]
```

- [x] **Step 5: Add color to timeline marker spans**

In `autolight/timeline/model.py`, import the display helper:

```python
from autolight.project.store import marker_display_color
```

Update `_marker_span`:

```python
    def _marker_span(self, marker: Marker) -> dict[str, str | float]:
        return {
            "id": marker.id,
            "timestamp": marker.timestamp,
            "duration": marker.duration or 0.0,
            "label": marker.label,
            "category": marker.category,
            "color": marker_display_color(marker),
        }
```

- [x] **Step 6: Run controller and timeline tests**

Run:

```bash
uv run python -m unittest tests.test_editable_marker_inspector tests.test_timeline_model -v
```

Expected: PASS.

- [x] **Step 7: Commit controller and timeline marker polish**

Run:

```bash
git add autolight/app_controller.py autolight/timeline/model.py tests/test_editable_marker_inspector.py tests/test_timeline_model.py
git commit -m "Expose polished marker editing state"
```

Expected: commit succeeds.

## Task 3: Marker Editing QML Polish

**Files:**
- Modify: `UI/Main.qml`
- Modify: `tests/test_editable_marker_inspector.py`

- [x] **Step 1: Add failing QML wiring test for labels, colors, and bulk edits**

Add this test to `tests/test_editable_marker_inspector.py`:

```python
    def test_qml_exposes_marker_label_color_and_bulk_edit_controls(self):
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")

        self.assertIn("id: markerColorPicker", qml)
        self.assertIn("id: markerCategoryField", qml)
        self.assertIn("appController.toggle_marker_selection", qml)
        self.assertIn("appController.update_selected_marker", qml)
        self.assertIn("appController.bulk_update_selected_markers", qml)
        self.assertIn("modelData.color", qml)
        self.assertIn("modelData.selected", qml)
        self.assertIn("selectedMarkerIds.length", qml)
```

- [x] **Step 2: Run QML marker wiring test and verify failure**

Run:

```bash
uv run python -m unittest tests.test_editable_marker_inspector.EditableMarkerInspectorTest.test_qml_exposes_marker_label_color_and_bulk_edit_controls -v
```

Expected: FAIL because the inspector does not expose color picker, category field, marker selection, or bulk update controls.

- [x] **Step 3: Add marker editor helper functions and color model**

In `UI/Main.qml`, add these root properties and functions after `statusError`:

```qml
    readonly property var markerColorOptions: [
        { key: "cyan", label: "Cyan", color: "#67e8f9" },
        { key: "green", label: "Green", color: "#a7f3d0" },
        { key: "amber", label: "Amber", color: "#fbbf24" },
        { key: "violet", label: "Violet", color: "#c4b5fd" },
        { key: "rose", label: "Rose", color: "#fda4af" },
        { key: "blue", label: "Blue", color: "#93c5fd" }
    ]

    function markerColorIndex(colorKey) {
        for (var i = 0; i < root.markerColorOptions.length; i++) {
            if (root.markerColorOptions[i].key === colorKey) {
                return i
            }
        }
        return 0
    }

    function selectedMarkerCount() {
        return appController.selectedMarkerIds.length
    }

    function syncMarkerEditor(marker) {
        markerTimestampField.text = Number(marker.timestamp).toFixed(2)
        markerLabelField.text = marker.label.length > 0 ? marker.label : "Cue"
        markerCategoryField.text = marker.category.length > 0 ? marker.category : "cue"
        markerColorPicker.currentIndex = root.markerColorIndex(marker.colorKey)
    }
```

- [x] **Step 4: Use resolved marker colors in the timeline lane**

Replace the marker rectangle color binding in the `markerSpans` repeater with:

```qml
                                color: modelData.color
```

Add a compact label inside the marker rectangle:

```qml
                                Text {
                                    anchors.centerIn: parent
                                    width: parent.width - 6
                                    text: modelData.label
                                    color: "#111318"
                                    font.pixelSize: 10
                                    font.bold: true
                                    horizontalAlignment: Text.AlignHCenter
                                    elide: Text.ElideRight
                                    visible: parent.width >= 36 && modelData.label.length > 0
                                }
```

- [x] **Step 5: Replace the simple inspector marker list with selectable marker rows**

Inside the `Repeater` for `appController.selectedTrackMarkers`, replace the delegate `Rectangle` body with:

```qml
                                delegate: Rectangle {
                                    required property var modelData
                                    width: markerList.width
                                    height: 34
                                    radius: 3
                                    color: modelData.selected ? "#2f4366" : "transparent"
                                    border.color: modelData.selected ? modelData.color : "transparent"

                                    Rectangle {
                                        id: markerColorSwatch
                                        width: 10
                                        height: 10
                                        radius: 5
                                        color: modelData.color
                                        anchors.left: parent.left
                                        anchors.leftMargin: 4
                                        anchors.verticalCenter: parent.verticalCenter
                                    }

                                    Text {
                                        anchors.left: markerColorSwatch.right
                                        anchors.leftMargin: 8
                                        anchors.right: parent.right
                                        anchors.rightMargin: 4
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: Number(modelData.timestamp).toFixed(2) + "  " + modelData.label
                                        color: "#f4f4f5"
                                        elide: Text.ElideRight
                                    }

                                    MouseArea {
                                        anchors.fill: parent
                                        onClicked: function(mouse) {
                                            appController.toggle_marker_selection(modelData.id, (mouse.modifiers & Qt.ShiftModifier) !== 0)
                                            root.syncMarkerEditor(modelData)
                                        }
                                    }
                                }
```

- [x] **Step 6: Add category, color, update, and bulk controls to the inspector**

Add this `TextField` after `markerLabelField`:

```qml
                    TextField {
                        id: markerCategoryField
                        placeholderText: "Category"
                        text: "cue"
                    }
```

Add this `ComboBox` after `markerCategoryField`:

```qml
                    ComboBox {
                        id: markerColorPicker
                        model: root.markerColorOptions
                        textRole: "label"
                        valueRole: "key"
                        width: parent.width
                        delegate: ItemDelegate {
                            width: markerColorPicker.width
                            text: modelData.label
                            contentItem: Row {
                                spacing: 8
                                Rectangle {
                                    width: 12
                                    height: 12
                                    radius: 6
                                    color: modelData.color
                                    anchors.verticalCenter: parent.verticalCenter
                                }
                                Text {
                                    text: modelData.label
                                    color: "#f4f4f5"
                                    anchors.verticalCenter: parent.verticalCenter
                                }
                            }
                        }
                    }
```

Update the existing add cue button call:

```qml
                        onClicked: appController.add_marker_to_selected_track(
                            Number(markerTimestampField.text),
                            markerLabelField.text,
                            markerCategoryField.text,
                            markerColorPicker.currentValue
                        )
```

Add these buttons below `Delete Cue`:

```qml
                    Button {
                        text: "Update Cue"
                        enabled: appController.selectedTrackIsEditable && root.selectedMarkerCount() === 1
                        onClicked: appController.update_selected_marker(
                            Number(markerTimestampField.text),
                            markerLabelField.text,
                            markerCategoryField.text,
                            markerColorPicker.currentValue
                        )
                    }

                    Button {
                        text: root.selectedMarkerCount() > 0 ? "Apply To Selected" : "Apply To Track"
                        enabled: appController.selectedTrackIsEditable && appController.selectedTrackMarkers.length > 0
                        onClicked: appController.bulk_update_selected_markers(
                            markerLabelField.text,
                            markerCategoryField.text,
                            markerColorPicker.currentValue
                        )
                    }
```

- [x] **Step 7: Update controller add-marker slot for metadata arguments**

In `autolight/app_controller.py`, change `add_marker_to_selected_track` to accept category and color while preserving Python callers with defaults:

```python
    @Slot(float, str, str, str, result=str)
    def add_marker_to_selected_track(
        self,
        timestamp: float,
        label: str,
        category: str = "cue",
        color: str = "cyan",
    ) -> str:
        try:
            marker = add_editable_marker(self._project, self._selected_track_id, timestamp, label)
            update_editable_marker(
                self._project,
                self._selected_track_id,
                marker.id,
                timestamp=marker.timestamp,
                label=marker.label,
                category=category,
                color=color,
            )
            self._track_model.set_project(self._project)
            self._set_selected_marker_ids([marker.id])
            self.selectedTrackMarkersChanged.emit()
            self._notify_timeline_duration_changed()
            self._set_last_error("")
            self._set_dirty(True)
            return marker.id
        except Exception as exc:
            self._set_last_error(str(exc))
            return ""
```

- [x] **Step 8: Run marker UI tests**

Run:

```bash
uv run python -m unittest tests.test_editable_marker_inspector tests.test_timeline_model -v
```

Expected: PASS.

- [x] **Step 9: Commit marker QML polish**

Run:

```bash
git add UI/Main.qml autolight/app_controller.py tests/test_editable_marker_inspector.py tests/test_timeline_model.py
git commit -m "Polish marker editing UI"
```

Expected: commit succeeds.

## Task 4: Waveform And Playback UX Polish With Visual QA

**Files:**
- Modify: `autolight/playback/transport.py`
- Modify: `autolight/app_controller.py`
- Modify: `UI/Main.qml`
- Modify: `main.py`
- Create: `scripts/check_qml_screenshot.py`
- Modify: `tests/test_playback_transport.py`
- Modify: `tests/test_app_controller.py`
- Modify: `tests/test_waveform_summary.py`

- [x] **Step 1: Add failing playback transport volume test**

Add this test to `tests/test_playback_transport.py`:

```python
    def test_set_volume_clamps_and_emits_volume_change(self):
        audio = FakeAudioOutput()
        transport = PlaybackTransport(player=FakeMediaPlayer(), audio_output=audio)
        changes = []
        transport.volumeChanged.connect(lambda: changes.append(transport.volume))

        transport.set_volume(1.5)
        transport.set_volume(0.25)
        transport.set_volume(-2.0)

        self.assertEqual(audio.volume, 0.0)
        self.assertEqual(changes, [0.25, 0.0])
        self.assertEqual(transport.volume, 0.0)
```

- [x] **Step 2: Add failing controller and QML playback UX tests**

Add these tests to `tests/test_app_controller.py`:

```python
    def test_nudge_playback_seeks_relative_to_current_position(self):
        controller = self._controller()
        controller.playback.load_source = Mock(return_value=True)
        controller.playback.play = Mock()

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path, frames=80000)
            controller.import_audio(str(audio_path))
            self.assertTrue(controller.play_selected_track())

        controller.seek_playback(2.0)
        controller.nudge_playback(1.5)

        self.assertEqual(controller.playback.positionSeconds, 3.5)

    def test_qml_exposes_polished_playback_and_waveform_controls(self):
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")

        self.assertIn("id: playbackControls", qml)
        self.assertIn("id: playbackVolumeSlider", qml)
        self.assertIn("appController.nudge_playback", qml)
        self.assertIn("appController.playback.set_volume", qml)
        self.assertIn("root.seekTimelineAtX", qml)
        self.assertIn("modelData.rms", qml)
        self.assertIn("id: waveformCenterLine", qml)
```

Add this test to `tests/test_waveform_summary.py`:

```python
    def test_qml_waveform_uses_peak_and_rms_layers(self):
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")

        self.assertIn("modelData.peak", qml)
        self.assertIn("modelData.rms", qml)
        self.assertIn("id: waveformCenterLine", qml)
```

- [x] **Step 3: Run playback UX tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_playback_transport.PlaybackTransportTest.test_set_volume_clamps_and_emits_volume_change tests.test_app_controller.AppControllerTest.test_nudge_playback_seeks_relative_to_current_position tests.test_app_controller.AppControllerTest.test_qml_exposes_polished_playback_and_waveform_controls tests.test_waveform_summary.WaveformSummaryTest.test_qml_waveform_uses_peak_and_rms_layers -v
```

Expected: FAIL because observable volume, nudge playback, and polished QML controls are missing.

- [x] **Step 4: Make playback volume observable**

In `autolight/playback/transport.py`, add the signal:

```python
    volumeChanged = Signal()
```

Initialize volume:

```python
        self._volume = 1.0
```

Add property:

```python
    @Property(float, notify=volumeChanged)
    def volume(self) -> float:
        return self._volume
```

Replace `set_volume()`:

```python
    @Slot(float)
    def set_volume(self, value: float) -> None:
        volume = min(max(self._finite_non_negative(value), 0.0), 1.0)
        self._audio_output.setVolume(volume)
        if self._volume == volume:
            return
        self._volume = volume
        self.volumeChanged.emit()
```

- [x] **Step 5: Add playback nudge slot**

In `autolight/app_controller.py`, add:

```python
    @Slot(float)
    def nudge_playback(self, delta_seconds: float) -> None:
        self.seek_playback(self._playback.positionSeconds + float(delta_seconds))
```

- [x] **Step 6: Add timeline seek helper and polished playback controls to QML**

In `UI/Main.qml`, add:

```qml
    function seekTimelineAtX(xValue) {
        var laneSeconds = appController.timelineScrollSeconds
            + Math.max(0, xValue - root.timelineLeftPadding) / appController.timelinePixelsPerSecond
        appController.seek_playback(Math.min(appController.timelineDurationSeconds, laneSeconds))
    }
```

Replace the existing Play, Stop, and time label toolbar controls with this grouped layout:

```qml
                RowLayout {
                    id: playbackControls
                    spacing: 6

                    Button {
                        text: "-1s"
                        enabled: appController.playback.sourcePath.length > 0
                        onClicked: appController.nudge_playback(-1.0)
                    }

                    Button {
                        text: appController.playback.isPlaying ? "Pause" : "Play"
                        enabled: appController.selectedTrackCanPlay || (appController.selectedTrackId.length === 0 && appController.playback.sourcePath.length > 0) || appController.playback.isPlaying
                        onClicked: root.togglePlayback()
                    }

                    Button {
                        text: "Stop"
                        enabled: appController.playback.sourcePath.length > 0
                        onClicked: appController.stop_playback()
                    }

                    Button {
                        text: "+1s"
                        enabled: appController.playback.sourcePath.length > 0
                        onClicked: appController.nudge_playback(1.0)
                    }

                    Slider {
                        id: playbackVolumeSlider
                        from: 0
                        to: 1
                        value: appController.playback.volume
                        Layout.preferredWidth: 96
                        onMoved: appController.playback.set_volume(value)
                    }

                    Label {
                        id: playheadTimeLabel
                        text: root.formatSeconds(appController.playback.positionSeconds) + " / " + root.formatSeconds(appController.playback.durationSeconds)
                        color: "#d4d4d8"
                        font.pixelSize: 12
                    }
                }
```

- [x] **Step 7: Render waveform peak and RMS layers**

Inside the lane rectangle that contains the waveform repeater, add a center line before the waveform `Repeater`:

```qml
                        Rectangle {
                            id: waveformCenterLine
                            x: root.timelineLeftPadding
                            y: Math.round(parent.height / 2)
                            width: Math.max(0, parent.width - root.timelineLeftPadding)
                            height: 1
                            color: "#2f333d"
                            visible: waveformSamples.length > 0
                        }
```

Replace the single waveform bar body with peak and RMS nested rectangles:

```qml
                            Item {
                                width: 3
                                height: parent.height
                                x: root.timelineX(index / Math.max(1, waveformSamples.length - 1) * waveformDurationSeconds)
                                visible: x >= root.timelineLeftPadding - width && x <= parent.width

                                Rectangle {
                                    width: 2
                                    height: Math.max(2, modelData.peak * (parent.height - 18))
                                    y: (parent.height - height) / 2
                                    color: "#60a5fa"
                                    opacity: 0.75
                                }

                                Rectangle {
                                    width: 2
                                    height: Math.max(2, modelData.rms * (parent.height - 18))
                                    y: (parent.height - height) / 2
                                    color: "#bfdbfe"
                                    opacity: 0.95
                                }
                            }
```

- [x] **Step 8: Seek when clicking timeline lanes**

Update the lane `MouseArea` click handler:

```qml
                        MouseArea {
                            anchors.fill: parent
                            acceptedButtons: Qt.LeftButton
                            onClicked: function(mouse) {
                                appController.select_track(trackId)
                                root.seekTimelineAtX(mouse.x)
                            }
                        }
```

- [x] **Step 9: Add deterministic screenshot mode**

In `main.py`, import `QTimer`:

```python
from PySide6.QtCore import QTimer
```

Add helper:

```python
def _argument_value(args: list[str], flag: str) -> str:
    try:
        return args[args.index(flag) + 1]
    except (ValueError, IndexError):
        return ""
```

In `main()`, replace the smoke return block with:

```python
        if not engine.rootObjects():
            return -1
        screenshot_path = _argument_value(args, "--screenshot")
        if screenshot_path:
            root = engine.rootObjects()[0]

            def capture() -> None:
                image = root.grabWindow()
                if not image.save(screenshot_path):
                    app.exit(2)
                    return
                app.exit(0)

            QTimer.singleShot(150, capture)
            return app.exec()
        if "--smoke" in args:
            return 0
        return app.exec()
```

- [x] **Step 10: Add QImage visual QA script**

Create `scripts/check_qml_screenshot.py`:

```python
import sys
from pathlib import Path

from PySide6.QtGui import QColor, QImage


TARGETS = {
    "playhead amber": QColor("#facc15"),
    "waveform blue": QColor("#60a5fa"),
    "marker cyan": QColor("#67e8f9"),
}


def close_enough(left: QColor, right: QColor, tolerance: int = 12) -> bool:
    return (
        abs(left.red() - right.red()) <= tolerance
        and abs(left.green() - right.green()) <= tolerance
        and abs(left.blue() - right.blue()) <= tolerance
        and left.alpha() > 0
    )


def count_color(image: QImage, target: QColor) -> int:
    count = 0
    for y in range(0, image.height(), 2):
        for x in range(0, image.width(), 2):
            if close_enough(QColor(image.pixelColor(x, y)), target):
                count += 1
    return count


def unique_sampled_colors(image: QImage) -> int:
    colors = set()
    for y in range(0, image.height(), 8):
        for x in range(0, image.width(), 8):
            color = QColor(image.pixelColor(x, y))
            if color.alpha() > 0:
                colors.add((color.red(), color.green(), color.blue()))
    return len(colors)


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print("usage: check_qml_screenshot.py SCREENSHOT.png", file=sys.stderr)
        return 2
    path = Path(argv[1])
    image = QImage(str(path))
    if image.isNull():
        print(f"could not load screenshot: {path}", file=sys.stderr)
        return 1
    if image.width() < 900 or image.height() < 600:
        print(f"screenshot is too small: {image.width()}x{image.height()}", file=sys.stderr)
        return 1
    if unique_sampled_colors(image) < 20:
        print("screenshot does not contain enough distinct UI colors", file=sys.stderr)
        return 1
    for label, color in TARGETS.items():
        if count_color(image, color) < 4:
            print(f"missing expected {label} region", file=sys.stderr)
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
```

- [x] **Step 11: Run playback and visual QA checks**

Run:

```bash
uv run python -m unittest tests.test_playback_transport tests.test_app_controller.AppControllerTest.test_nudge_playback_seeks_relative_to_current_position tests.test_app_controller.AppControllerTest.test_qml_exposes_polished_playback_and_waveform_controls tests.test_waveform_summary.WaveformSummaryTest.test_qml_waveform_uses_peak_and_rms_layers -v
QT_QPA_PLATFORM=offscreen uv run python main.py --screenshot /tmp/autolight-ui-ux-polish.png
uv run python scripts/check_qml_screenshot.py /tmp/autolight-ui-ux-polish.png
```

Expected: unit tests PASS, screenshot command exits 0, and pixel check exits 0.

- [x] **Step 12: Commit playback and visual QA polish**

Run:

```bash
git add autolight/playback/transport.py autolight/app_controller.py UI/Main.qml main.py scripts/check_qml_screenshot.py tests/test_playback_transport.py tests/test_app_controller.py tests/test_waveform_summary.py
git commit -m "Polish waveform playback inspection"
```

Expected: commit succeeds.

## Task 5: Persist Timeline Zoom, Scroll, And Selected Track

**Files:**
- Modify: `autolight/app_controller.py`
- Modify: `tests/test_app_controller.py`
- Modify: `README.md`

- [x] **Step 1: Add failing UI-state persistence tests**

Add these tests to `tests/test_app_controller.py`:

```python
    def test_save_and_open_restores_timeline_zoom_scroll_and_selected_track(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            project_path = root / "show.autolight"
            write_wav(audio_path, frames=80000)
            source_id = controller.import_audio(str(audio_path))
            generated_id = controller.add_fixed_interval_track(source_id, 10.0, 0.5)
            controller.select_track(generated_id)
            controller.set_timeline_visible_seconds(4.0)
            controller.set_timeline_zoom(144.0)
            controller.set_timeline_scroll_seconds(3.0)

            self.assertTrue(controller.save_project(str(project_path)))

            reopened = self._controller()
            self.assertTrue(reopened.open_project(str(project_path)))

        self.assertEqual(reopened.selectedTrackId, generated_id)
        self.assertEqual(reopened.timelinePixelsPerSecond, 144.0)
        self.assertEqual(reopened.timelineScrollSeconds, 3.0)

    def test_open_project_ignores_missing_selected_track_in_ui_state(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            project_path = root / "show.autolight"
            write_wav(audio_path)
            controller.import_audio(str(audio_path))
            controller._project.ui_state["timeline"] = {
                "selected_track_id": "missing_track",
                "pixels_per_second": 120.0,
                "scroll_seconds": 1.0,
            }
            self.assertTrue(controller.save_project(str(project_path)))

            reopened = self._controller()
            self.assertTrue(reopened.open_project(str(project_path)))

        self.assertEqual(reopened.selectedTrackId, "")
        self.assertEqual(reopened.lastError, "")

    def test_ui_state_values_are_clamped_when_restored(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            project_path = root / "show.autolight"
            write_wav(audio_path)
            controller.import_audio(str(audio_path))
            controller._project.ui_state["timeline"] = {
                "selected_track_id": controller.selectedTrackId,
                "pixels_per_second": 999.0,
                "scroll_seconds": -10.0,
            }
            self.assertTrue(controller.save_project(str(project_path)))

            reopened = self._controller()
            self.assertTrue(reopened.open_project(str(project_path)))

        self.assertEqual(reopened.timelinePixelsPerSecond, 240.0)
        self.assertEqual(reopened.timelineScrollSeconds, 0.0)
```

- [x] **Step 2: Run UI-state tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_save_and_open_restores_timeline_zoom_scroll_and_selected_track tests.test_app_controller.AppControllerTest.test_open_project_ignores_missing_selected_track_in_ui_state tests.test_app_controller.AppControllerTest.test_ui_state_values_are_clamped_when_restored -v
```

Expected: FAIL because timeline state is not captured before save or restored on open.

- [x] **Step 3: Capture timeline UI state before saving**

In `autolight/app_controller.py`, add:

```python
TIMELINE_UI_STATE_KEY = "timeline"
```

Add helper:

```python
    def _capture_timeline_ui_state(self) -> None:
        self._project.ui_state[TIMELINE_UI_STATE_KEY] = {
            "selected_track_id": self._selected_track_id,
            "pixels_per_second": self._timeline_pixels_per_second,
            "scroll_seconds": self._timeline_scroll_seconds,
        }
```

Call it in `save_project()` immediately before `ProjectStore.save(...)`:

```python
            self._capture_timeline_ui_state()
            ProjectStore.save(self._project, project_path)
```

- [x] **Step 4: Restore timeline UI state during open**

Add helpers:

```python
    def _restore_timeline_ui_state(self) -> None:
        state = self._project.ui_state.get(TIMELINE_UI_STATE_KEY, {})
        if not isinstance(state, dict):
            return
        pixels_per_second = state.get("pixels_per_second")
        if pixels_per_second is not None:
            self.set_timeline_zoom(float(pixels_per_second))
        selected_track_id = state.get("selected_track_id", "")
        if isinstance(selected_track_id, str) and find_track(self._project, selected_track_id) is not None:
            self._set_selected_track_id(selected_track_id)
        else:
            self._set_selected_track_id("")
        scroll_seconds = state.get("scroll_seconds")
        if scroll_seconds is not None:
            self.set_timeline_scroll_seconds(float(scroll_seconds))
```

Change `_set_project` to accept a restore flag:

```python
    def _set_project(self, project, *, restore_ui_state: bool = False) -> None:
```

At the end of `_set_project`, after `_notify_timeline_duration_changed()` and `timelineScrollSecondsChanged.emit()`, add:

```python
        if restore_ui_state:
            self._restore_timeline_ui_state()
```

Update `open_project()` to call:

```python
            self._set_project(project, restore_ui_state=True)
```

Remove the unconditional selected-track clearing line from `open_project()`:

```python
            self._set_selected_track_id("")
```

Keep `new_project()` and `load_demo_project()` on the default `restore_ui_state=False`.

- [x] **Step 5: Document persisted timeline context**

In `README.md`, add a bullet under `Current Scope`:

```markdown
- Restore saved timeline zoom, horizontal scroll, and selected track when reopening a project.
```

Add this sentence after the Basic Workflow list:

```markdown
Timeline zoom, horizontal scroll, and the selected track are stored in the `.autolight` project when you save.
```

- [x] **Step 6: Run UI-state persistence tests**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_save_and_open_restores_timeline_zoom_scroll_and_selected_track tests.test_app_controller.AppControllerTest.test_open_project_ignores_missing_selected_track_in_ui_state tests.test_app_controller.AppControllerTest.test_ui_state_values_are_clamped_when_restored -v
```

Expected: PASS.

- [x] **Step 7: Commit timeline persistence**

Run:

```bash
git add autolight/app_controller.py tests/test_app_controller.py README.md
git commit -m "Persist timeline viewport state"
```

Expected: commit succeeds.

## Task 6: Overall UI Polish Pass

**Files:**
- Modify: `UI/Main.qml`
- Modify: `tests/test_app_controller.py`
- Modify: `tests/test_editable_marker_inspector.py`
- Modify: `README.md`

- [x] **Step 1: Add failing QML polish assertions**

Add this test to `tests/test_app_controller.py`:

```python
    def test_qml_uses_grouped_toolbar_and_stable_lane_dimensions(self):
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")

        self.assertIn("id: fileActions", qml)
        self.assertIn("id: transformActions", qml)
        self.assertIn("id: timelineControls", qml)
        self.assertIn("readonly property int timelineRowHeight", qml)
        self.assertIn("height: root.timelineRowHeight", qml)
        self.assertIn("elide: Text.ElideRight", qml)
        self.assertIn("clip: true", qml)
```

Add this assertion to `test_qml_exposes_marker_label_color_and_bulk_edit_controls`:

```python
        self.assertIn("No track selected", qml)
```

- [x] **Step 2: Run QML polish assertions and verify failure**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_qml_uses_grouped_toolbar_and_stable_lane_dimensions tests.test_editable_marker_inspector.EditableMarkerInspectorTest.test_qml_exposes_marker_label_color_and_bulk_edit_controls -v
```

Expected: FAIL until the toolbar is grouped, lane height is centralized, and inspector empty states are present.

- [x] **Step 3: Add theme and stable layout constants**

In `UI/Main.qml`, add these root constants:

```qml
    readonly property int timelineRowHeight: 76
    readonly property int compactButtonHeight: 30
    readonly property color panelBackground: "#1c1f26"
    readonly property color laneBackground: "#171a20"
    readonly property color laneBackgroundAlt: "#14171d"
    readonly property color borderSubtle: "#2f333d"
    readonly property color textPrimary: "#f4f4f5"
    readonly property color textMuted: "#a1a1aa"
    readonly property color focusAccent: "#facc15"
```

Replace hard-coded delegate height:

```qml
                    height: root.timelineRowHeight
```

Replace repeated lane and border colors in the timeline row with root constants:

```qml
                        color: index % 2 === 0 ? root.laneBackground : root.laneBackgroundAlt
                        border.color: appController.selectedTrackId === trackId ? root.focusAccent : root.borderSubtle
```

- [x] **Step 4: Split the crowded toolbar into grouped actions**

In `UI/Main.qml`, replace the toolbar row after the project title and spacer with three grouped `RowLayout`s:

```qml
                RowLayout {
                    id: fileActions
                    spacing: 6

                    Button { text: "New"; onClicked: root.newProjectWithConfirmation() }
                    Button { text: "Open"; onClicked: openProjectDialog.open() }
                    Button { text: "Save"; onClicked: appController.projectPath.length > 0 ? appController.save_project("") : saveProjectDialog.open() }
                    Button { text: "Save As"; onClicked: saveProjectDialog.open() }
                    Button { text: "Demo"; onClicked: root.demoProjectWithConfirmation() }
                }

                RowLayout {
                    id: transformActions
                    spacing: 6

                    Button {
                        text: "Import Audio"
                        onClicked: importAudioDialog.open()
                    }

                    Button {
                        text: "Add Markers"
                        enabled: appController.selectedTrackId.length > 0
                        onClicked: appController.add_fixed_interval_track(appController.selectedTrackId, root.defaultMarkerDuration, root.defaultMarkerInterval)
                    }

                    Button {
                        text: "Run"
                        enabled: appController.selectedTrackCanRerun && !appController.selectedTrackHasRunningJob
                        onClicked: appController.run_track(appController.selectedTrackId)
                    }

                    Button {
                        text: "Rerun"
                        enabled: appController.selectedTrackCanRerun && !appController.selectedTrackHasRunningJob
                        onClicked: appController.rerun_track(appController.selectedTrackId)
                    }

                    Button {
                        text: "Cancel"
                        enabled: appController.selectedTrackHasRunningJob
                        onClicked: appController.cancel_selected_job()
                    }
                }
```

Keep `transformPicker`, `transformParamsField`, `Add Transform`, `Add Vocals Stem`, `Check Cache`, and `Derive Editable` in a second `RowLayout` directly below the main `ToolBar`:

```qml
        RowLayout {
            id: transformDetailBar
            Layout.fillWidth: true
            Layout.leftMargin: 12
            Layout.rightMargin: 12
            Layout.topMargin: 6
            Layout.bottomMargin: 6
            spacing: 8

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

            Button {
                text: "Add Vocals Stem"
                enabled: appController.selectedTrackId.length > 0
                onClicked: appController.add_vocals_stem_track(appController.selectedTrackId)
            }

            Button {
                text: "Check Cache"
                onClicked: appController.refresh_cache_status()
            }

            Button {
                text: "Derive Editable"
                enabled: appController.selectedTrackId.length > 0
                onClicked: appController.create_editable_track_from_track(appController.selectedTrackId)
            }

            Item { Layout.fillWidth: true }
        }
```

- [x] **Step 5: Name timeline control group and improve empty inspector state**

Set the zoom/scroll `RowLayout` id:

```qml
        RowLayout {
            id: timelineControls
```

At the top of the inspector `Column`, under the `Inspector` label, add:

```qml
                    Text {
                        text: appController.selectedTrackId.length === 0 ? "No track selected" : ""
                        visible: appController.selectedTrackId.length === 0
                        color: root.textMuted
                        font.pixelSize: 12
                        wrapMode: Text.WordWrap
                        width: parent.width
                    }
```

Ensure inspector text fields and buttons remain disabled unless the selected track is editable:

```qml
                        enabled: appController.selectedTrackIsEditable
```

Apply this property to `markerTimestampField`, `markerLabelField`, `markerCategoryField`, and `markerColorPicker`.

- [x] **Step 6: Run QML polish tests**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_qml_uses_grouped_toolbar_and_stable_lane_dimensions tests.test_editable_marker_inspector.EditableMarkerInspectorTest.test_qml_exposes_marker_label_color_and_bulk_edit_controls -v
```

Expected: PASS.

- [x] **Step 7: Run visual QA after layout polish**

Run:

```bash
QT_QPA_PLATFORM=offscreen uv run python main.py --screenshot /tmp/autolight-ui-polish-final.png
uv run python scripts/check_qml_screenshot.py /tmp/autolight-ui-polish-final.png
```

Expected: both commands exit 0.

- [x] **Step 8: Commit UI polish**

Run:

```bash
git add UI/Main.qml tests/test_app_controller.py tests/test_editable_marker_inspector.py README.md
git commit -m "Polish Autolight UI layout"
```

Expected: commit succeeds.

## Task 7: Final Verification And Plan Closure

**Files:**
- Modify: `docs/superpowers/plans/2026-06-01-autolight-ui-ux-polish.md`

- [x] **Step 1: Run the focused suites from this plan**

Run:

```bash
uv run python -m unittest tests.test_editable_marker_inspector tests.test_timeline_model tests.test_playback_transport tests.test_waveform_summary tests.test_app_controller -v
```

Expected: PASS.

- [x] **Step 2: Run full test suite**

Run:

```bash
uv run python -m unittest discover -s tests -v
```

Expected: PASS.

- [x] **Step 3: Run app smoke and visual QA**

Run:

```bash
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
QT_QPA_PLATFORM=offscreen uv run python main.py --screenshot /tmp/autolight-ui-ux-polish-final.png
uv run python scripts/check_qml_screenshot.py /tmp/autolight-ui-ux-polish-final.png
```

Expected: all commands exit 0.

- [x] **Step 4: Check whitespace and worktree**

Run:

```bash
git diff --check
git status --short
```

Expected: `git diff --check` exits 0. `git status --short` shows only intended files before the final docs commit.

- [x] **Step 5: Mark this plan complete**

After all previous steps pass, update every completed checkbox in `docs/superpowers/plans/2026-06-01-autolight-ui-ux-polish.md` to checked.

- [x] **Step 6: Commit plan closure**

Run:

```bash
git add docs/superpowers/plans/2026-06-01-autolight-ui-ux-polish.md
git commit -m "Close UI UX polish plan"
```

Expected: commit succeeds.

## Implementation Notes

- Store marker colors as palette keys in `Marker.metadata["color"]`; expose resolved hex colors only to UI-facing summaries and roles.
- Keep generated tracks read-only. All marker mutation helpers must reject non-editable tracks.
- Empty marker ID lists in `bulk_update_editable_markers()` mean "all markers on the selected editable track"; non-empty lists mean "only these marker IDs".
- Persist zoom, scroll, and selected track only when the project is saved. Do not persist playback position in `.autolight` files.
- Screenshot visual QA must use the demo project loaded by `main.py`, because it provides predictable source, generated, and editable tracks.
- If offscreen screenshot capture is blank on a local Qt backend, keep the screenshot command in the plan but debug the capture mechanism before weakening the pixel assertions.

## Self-Review

- Requested marker labels, colors, and bulk edits are covered by Tasks 1 through 3.
- Requested waveform/playback UX polish with visual QA is covered by Task 4 and final visual checks in Task 7.
- Requested timeline zoom, scroll, and selected track persistence is covered by Task 5.
- Requested UI polish pass is covered by Task 6.
- The plan uses existing project boundaries and avoids changing the `.autolight` schema version because `ui_state` and `Marker.metadata` already exist for extensible UI data.
