# Autolight Interactive Timeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Milestone 2: direct editable timeline authoring, manual cue tracks, undo/redo, smoother playback follow, componentized QML, and zoom-adaptive waveform detail.

**Architecture:** Keep `AppController` as the QML-facing facade, but move project lifecycle, marker editing, edit history, viewport policy, and waveform LOD into focused `autolight/app/` modules. Split `UI/Main.qml` into composable QML files under `UI/components/`, with timeline lanes rendering marker and waveform content through localized components instead of one monolithic root file. Preserve existing public behavior while adding new slots/properties for manual tracks, direct marker edits, snapping, undo/redo, and waveform visible slices.

**Tech Stack:** Python 3.14, PySide6/QML, PySide6 `QtMultimedia`, `unittest`, JSON cache artifacts, existing `ProjectStore`, `TimelineTrackModel`, `TransformRegistry`, `LocalJobQueue`, `PlaybackTransport`, and offscreen QML smoke/screenshot checks.

---

## File Structure

- Create `autolight/app/__init__.py`: exports the new app-layer helper classes used by `AppController`.
- Create `autolight/app/session.py`: project replacement, path, dirty-state, demo temp cleanup, and save/open/import helpers that do not own Qt signals.
- Create `autolight/app/marker_editing.py`: manual track creation, source-track resolution, marker add/delete/update/move/resize, snap target selection, and atomic validation.
- Create `autolight/app/edit_history.py`: undo/redo stack and command objects for project mutations.
- Create `autolight/app/timeline_viewport.py`: zoom/scroll/visible-duration model, follow throttling, edge bands, and zoom anchoring.
- Create `autolight/app/waveform_lod.py`: waveform payload parsing, legacy payload fallback, level selection, visible slicing, and QML-friendly sample conversion.
- Modify `autolight/app_controller.py`: keep Qt signals/properties/slots, delegate behavior into app-layer units, add new editing/history/waveform slots and properties.
- Modify `autolight/project/store.py`: add low-level manual editable track, marker move, marker resize, and marker snapshot helpers used by app-layer commands.
- Modify `autolight/project/__init__.py`: export new store helpers.
- Modify `autolight/analysis/waveform.py`: generate waveform pyramid payloads while preserving legacy sample compatibility.
- Modify `autolight/analysis/builtin.py`: update waveform transform metadata for the pyramid payload.
- Modify `autolight/timeline/model.py`: expose editability, selected marker state, marker duration, waveform level metadata, and visible waveform samples as needed by QML.
- Create `UI/components/ProjectToolbar.qml`
- Create `UI/components/TransformBar.qml`
- Create `UI/components/PlaybackBar.qml`
- Create `UI/components/TimelineRuler.qml`
- Create `UI/components/TimelineView.qml`
- Create `UI/components/TrackRow.qml`
- Create `UI/components/TimelineLane.qml`
- Create `UI/components/MarkerBlock.qml`
- Create `UI/components/WaveformStrip.qml`
- Create `UI/components/MarkerInspector.qml`
- Create `UI/components/StatusFooter.qml`
- Modify `UI/Main.qml`: compose the new components and keep dialogs/root constants only.
- Modify `UI/qmldir`: register QML component modules if imports require explicit module entries.
- Modify `tests/test_app_controller.py`: controller slots, history state, viewport follow behavior, QML wiring, and file-size/componentization assertions.
- Modify `tests/test_editable_marker_inspector.py`: manual track, direct marker move/resize, snapping, and undo/redo integration tests.
- Modify `tests/test_timeline_model.py`: editability, marker duration/selection, visible waveform role tests.
- Modify `tests/test_waveform_summary.py`: waveform pyramid generation, level selection, visible slicing, and legacy fallback tests.
- Modify `tests/test_playback_transport.py`: keep existing playback tests aligned if follow policy needs transport signal changes.
- Modify `README.md`: describe manual cue tracks, direct editing, undo/redo, snapping, and zoom-adaptive waveforms.
- Modify `docs/superpowers/plans/2026-06-01-autolight-interactive-timeline.md`: mark completed steps as implementation proceeds.

## Task 1: App-Layer Module Shell And Controller Facade Boundary

**Files:**
- Create: `autolight/app/__init__.py`
- Create: `autolight/app/session.py`
- Modify: `autolight/app_controller.py`
- Modify: `tests/test_app_controller.py`

- [x] **Step 1: Add failing structure tests for the app-layer boundary**

Add these imports near the top of `tests/test_app_controller.py`:

```python
import importlib
```

Add these tests to `AppControllerTest`:

```python
    def test_app_layer_modules_exist_for_milestone_2_boundaries(self):
        module_names = [
            "autolight.app.session",
            "autolight.app.marker_editing",
            "autolight.app.edit_history",
            "autolight.app.timeline_viewport",
            "autolight.app.waveform_lod",
        ]

        for module_name in module_names:
            with self.subTest(module_name=module_name):
                self.assertIsNotNone(importlib.import_module(module_name))

    def test_app_controller_constructs_app_layer_collaborators(self):
        controller = self._controller()

        self.assertEqual(type(controller._session).__name__, "ProjectSession")
        self.assertEqual(type(controller._edit_history).__name__, "EditHistory")
        self.assertEqual(type(controller._viewport).__name__, "TimelineViewport")
        self.assertEqual(type(controller._waveform_lod).__name__, "WaveformLodStore")
```

- [x] **Step 2: Run structure tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_app_layer_modules_exist_for_milestone_2_boundaries tests.test_app_controller.AppControllerTest.test_app_controller_constructs_app_layer_collaborators -v
```

Expected: FAIL with `ModuleNotFoundError` for `autolight.app.session` or `AttributeError` for missing controller collaborators.

- [x] **Step 3: Create the app-layer package and minimal session boundary**

Create `autolight/app/__init__.py`:

```python
from autolight.app.edit_history import EditHistory
from autolight.app.marker_editing import MarkerEditingService
from autolight.app.session import ProjectSession
from autolight.app.timeline_viewport import TimelineViewport
from autolight.app.waveform_lod import WaveformLodStore

__all__ = [
    "EditHistory",
    "MarkerEditingService",
    "ProjectSession",
    "TimelineViewport",
    "WaveformLodStore",
]
```

Create `autolight/app/session.py`:

```python
from __future__ import annotations

import tempfile
from dataclasses import dataclass

from autolight.project.models import ProjectDocument
from autolight.project.store import new_project


@dataclass(slots=True)
class ProjectSession:
    project: ProjectDocument
    project_path: str = ""
    dirty: bool = False
    demo_temp_dir: tempfile.TemporaryDirectory | None = None

    @classmethod
    def empty(cls) -> "ProjectSession":
        return cls(project=new_project("Untitled"))

    def replace_project(
        self,
        project: ProjectDocument,
        *,
        project_path: str = "",
        dirty: bool = False,
    ) -> None:
        self.cleanup_demo()
        self.project = project
        self.project_path = project_path
        self.dirty = dirty

    def set_dirty(self, dirty: bool) -> bool:
        if self.dirty == dirty:
            return False
        self.dirty = dirty
        return True

    def set_project_path(self, path: str) -> bool:
        if self.project_path == path:
            return False
        self.project_path = path
        return True

    def cleanup_demo(self) -> None:
        if self.demo_temp_dir is None:
            return
        self.demo_temp_dir.cleanup()
        self.demo_temp_dir = None
```

Create minimal modules that satisfy construction now and receive behavior in the following tasks.

`autolight/app/edit_history.py`:

```python
from __future__ import annotations


class EditHistory:
    def __init__(self):
        self._undo_stack = []
        self._redo_stack = []

    @property
    def can_undo(self) -> bool:
        return bool(self._undo_stack)

    @property
    def can_redo(self) -> bool:
        return bool(self._redo_stack)

    def clear(self) -> None:
        self._undo_stack.clear()
        self._redo_stack.clear()
```

`autolight/app/marker_editing.py`:

```python
from __future__ import annotations


class MarkerEditingService:
    def __init__(self):
        self.snap_threshold_pixels = 8.0
```

`autolight/app/timeline_viewport.py`:

```python
from __future__ import annotations


class TimelineViewport:
    def __init__(self):
        self.follow_edge_fraction = 0.20
```

`autolight/app/waveform_lod.py`:

```python
from __future__ import annotations


class WaveformLodStore:
    def __init__(self):
        self.target_pixels_per_bucket = 4.0
```

In `autolight/app_controller.py`, import and initialize the collaborators:

```python
from autolight.app import (
    EditHistory,
    MarkerEditingService,
    ProjectSession,
    TimelineViewport,
    WaveformLodStore,
)
```

Inside `AppController.__init__`, after creating `self._project`:

```python
        self._session = ProjectSession(self._project)
        self._marker_editing = MarkerEditingService()
        self._edit_history = EditHistory()
        self._viewport = TimelineViewport()
        self._waveform_lod = WaveformLodStore()
```

- [x] **Step 4: Run structure tests**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_app_layer_modules_exist_for_milestone_2_boundaries tests.test_app_controller.AppControllerTest.test_app_controller_constructs_app_layer_collaborators -v
```

Expected: PASS.

- [x] **Step 5: Commit app-layer shell**

```bash
git add autolight/app tests/test_app_controller.py autolight/app_controller.py
git commit -m "Add app-layer boundaries for interactive timeline"
```

Expected: commit succeeds.

## Task 2: Timeline Viewport Policy And Playback Follow Throttling

**Files:**
- Modify: `autolight/app/timeline_viewport.py`
- Modify: `autolight/app_controller.py`
- Modify: `tests/test_app_controller.py`

- [x] **Step 1: Add failing viewport policy tests**

Add these tests to `AppControllerTest`:

```python
    def test_playback_follow_updates_scroll_only_when_playhead_enters_edge_band(self):
        controller = self._controller()
        controller.set_timeline_visible_seconds(10.0)
        controller.set_timeline_scroll_seconds(20.0)

        next_scroll = controller._viewport.scroll_for_follow(
            position_seconds=25.0,
            scroll_seconds=20.0,
            visible_seconds=10.0,
            duration_seconds=60.0,
        )
        self.assertEqual(next_scroll, 20.0)

        next_scroll = controller._viewport.scroll_for_follow(
            position_seconds=29.5,
            scroll_seconds=20.0,
            visible_seconds=10.0,
            duration_seconds=60.0,
        )
        self.assertGreater(next_scroll, 20.0)
        self.assertLessEqual(next_scroll, 50.0)

    def test_playback_follow_throttles_scroll_updates(self):
        controller = self._controller()
        controller.set_timeline_visible_seconds(10.0)
        controller.set_timeline_scroll_seconds(0.0)

        self.assertTrue(controller._viewport.should_emit_follow_scroll(0.000))
        self.assertFalse(controller._viewport.should_emit_follow_scroll(0.010))
        self.assertTrue(controller._viewport.should_emit_follow_scroll(0.034))

    def test_zoom_around_anchor_keeps_anchor_screen_position_stable(self):
        controller = self._controller()
        controller.set_timeline_visible_seconds(10.0)
        controller.set_timeline_scroll_seconds(4.0)

        zoom, scroll = controller._viewport.zoom_around_anchor(
            current_zoom=100.0,
            requested_zoom=200.0,
            current_scroll=4.0,
            visible_seconds=10.0,
            duration_seconds=30.0,
            anchor_seconds=6.0,
        )

        self.assertEqual(zoom, 200.0)
        self.assertAlmostEqual(scroll, 5.0)
```

- [x] **Step 2: Run viewport tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_playback_follow_updates_scroll_only_when_playhead_enters_edge_band tests.test_app_controller.AppControllerTest.test_playback_follow_throttles_scroll_updates tests.test_app_controller.AppControllerTest.test_zoom_around_anchor_keeps_anchor_screen_position_stable -v
```

Expected: FAIL because `TimelineViewport` does not yet implement the policy methods.

- [x] **Step 3: Implement viewport policy**

Replace `autolight/app/timeline_viewport.py` with:

```python
from __future__ import annotations

import math


TIMELINE_MIN_PIXELS_PER_SECOND = 24.0
TIMELINE_MAX_PIXELS_PER_SECOND = 240.0
FOLLOW_EDGE_FRACTION = 0.20
FOLLOW_MAX_HZ = 30.0


class TimelineViewport:
    def __init__(self):
        self._last_follow_emit_seconds = -math.inf

    def clamp_zoom(self, pixels_per_second: float) -> float:
        value = self._finite_positive(pixels_per_second, fallback=96.0)
        return min(max(value, TIMELINE_MIN_PIXELS_PER_SECOND), TIMELINE_MAX_PIXELS_PER_SECOND)

    def clamp_scroll(
        self,
        scroll_seconds: float,
        *,
        visible_seconds: float,
        duration_seconds: float,
    ) -> float:
        value = self._finite_non_negative(scroll_seconds)
        max_scroll = max(0.0, self._finite_non_negative(duration_seconds) - max(0.01, visible_seconds))
        return min(value, max_scroll)

    def scroll_for_follow(
        self,
        *,
        position_seconds: float,
        scroll_seconds: float,
        visible_seconds: float,
        duration_seconds: float,
    ) -> float:
        visible = max(0.01, self._finite_positive(visible_seconds, fallback=0.01))
        current = self.clamp_scroll(
            scroll_seconds,
            visible_seconds=visible,
            duration_seconds=duration_seconds,
        )
        position = self._finite_non_negative(position_seconds)
        leading_edge = current + visible * FOLLOW_EDGE_FRACTION
        trailing_edge = current + visible * (1.0 - FOLLOW_EDGE_FRACTION)
        if position < leading_edge:
            target = position - visible * FOLLOW_EDGE_FRACTION
        elif position > trailing_edge:
            target = position - visible * (1.0 - FOLLOW_EDGE_FRACTION)
        else:
            target = current
        return self.clamp_scroll(target, visible_seconds=visible, duration_seconds=duration_seconds)

    def should_emit_follow_scroll(self, now_seconds: float) -> bool:
        now = self._finite_non_negative(now_seconds)
        minimum_interval = 1.0 / FOLLOW_MAX_HZ
        if now - self._last_follow_emit_seconds < minimum_interval:
            return False
        self._last_follow_emit_seconds = now
        return True

    def zoom_around_anchor(
        self,
        *,
        current_zoom: float,
        requested_zoom: float,
        current_scroll: float,
        visible_seconds: float,
        duration_seconds: float,
        anchor_seconds: float,
    ) -> tuple[float, float]:
        old_zoom = self.clamp_zoom(current_zoom)
        new_zoom = self.clamp_zoom(requested_zoom)
        visible = max(0.01, self._finite_positive(visible_seconds, fallback=0.01))
        scroll = self.clamp_scroll(
            current_scroll,
            visible_seconds=visible,
            duration_seconds=duration_seconds,
        )
        anchor = self._finite_non_negative(anchor_seconds)
        anchor_fraction = 0.0 if visible <= 0.0 else (anchor - scroll) / visible
        new_visible = visible * old_zoom / new_zoom
        new_scroll = anchor - anchor_fraction * new_visible
        return new_zoom, self.clamp_scroll(
            new_scroll,
            visible_seconds=new_visible,
            duration_seconds=duration_seconds,
        )

    @staticmethod
    def _finite_non_negative(value: float) -> float:
        number = float(value)
        return number if math.isfinite(number) and number >= 0.0 else 0.0

    @staticmethod
    def _finite_positive(value: float, *, fallback: float) -> float:
        number = float(value)
        return number if math.isfinite(number) and number > 0.0 else fallback
```

Update `AppController.set_timeline_zoom()` to use the policy:

```python
        anchor = (
            self._playback.positionSeconds
            if self._playback.property("sourcePath")
            else self._timeline_scroll_seconds + self._visible_timeline_seconds() / 2
        )
        clamped, next_scroll = self._viewport.zoom_around_anchor(
            current_zoom=self._timeline_pixels_per_second,
            requested_zoom=pixels_per_second,
            current_scroll=self._timeline_scroll_seconds,
            visible_seconds=self._visible_timeline_seconds(),
            duration_seconds=self._timeline_duration_seconds(),
            anchor_seconds=anchor,
        )
```

After emitting `timelinePixelsPerSecondChanged`, assign `next_scroll` through `set_timeline_scroll_seconds(next_scroll)`.

Update `_keep_playback_position_visible()`:

```python
        if not self._playback.isPlaying:
            return
        next_scroll = self._viewport.scroll_for_follow(
            position_seconds=self._playback.positionSeconds,
            scroll_seconds=self._timeline_scroll_seconds,
            visible_seconds=self._visible_timeline_seconds(),
            duration_seconds=self._timeline_duration_seconds(),
        )
        if next_scroll == self._timeline_scroll_seconds:
            return
        if self._viewport.should_emit_follow_scroll(time.monotonic()):
            self.set_timeline_scroll_seconds(next_scroll)
```

Add `import time` to `autolight/app_controller.py`.

- [x] **Step 4: Run viewport tests**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_playback_follow_updates_scroll_only_when_playhead_enters_edge_band tests.test_app_controller.AppControllerTest.test_playback_follow_throttles_scroll_updates tests.test_app_controller.AppControllerTest.test_zoom_around_anchor_keeps_anchor_screen_position_stable -v
```

Expected: PASS.

- [x] **Step 5: Run playback and app-controller focused suites**

Run:

```bash
uv run python -m unittest tests.test_playback_transport tests.test_app_controller -v
```

Expected: PASS.

- [x] **Step 6: Commit viewport policy**

```bash
git add autolight/app/timeline_viewport.py autolight/app_controller.py tests/test_app_controller.py
git commit -m "Add timeline viewport follow policy"
```

Expected: commit succeeds.

## Task 3: Manual Cue Tracks And Atomic Marker Editing Helpers

**Files:**
- Modify: `autolight/project/store.py`
- Modify: `autolight/project/__init__.py`
- Modify: `autolight/app/marker_editing.py`
- Modify: `tests/test_editable_marker_inspector.py`

- [x] **Step 1: Add failing manual-track and marker-editing tests**

Add these imports to `tests/test_editable_marker_inspector.py`:

```python
from autolight.app.marker_editing import MarkerEditingService
from autolight.project.models import TrackType
from autolight.project.store import create_manual_editable_track, move_editable_markers, resize_editable_marker
```

Add these tests to `EditableMarkerInspectorTest`:

```python
    def test_create_manual_editable_track_uses_resolved_source_track(self):
        project = new_project("Demo")
        source = self._source_track(project)
        generated = add_generated_track(
            project,
            source.id,
            "Generated",
            "markers.fixed_interval",
            {},
            "1",
            "markers.v1",
            "dep",
        )

        manual = create_manual_editable_track(project, generated.id, "Manual Cues")

        self.assertEqual(manual.type, TrackType.EDITABLE)
        self.assertEqual(manual.input_track_ids, [source.id])
        self.assertEqual(manual.result_state, ResultState.COMPLETE)
        self.assertEqual(manual.provenance["manual_track"], True)
        self.assertEqual(manual.provenance["created_by"], "user")

    def test_create_manual_editable_track_rejects_track_without_source_context(self):
        project = new_project("Demo")

        with self.assertRaisesRegex(ValueError, "source audio"):
            create_manual_editable_track(project, "", "Manual Cues")

    def test_move_editable_markers_is_atomic_for_negative_result(self):
        project = new_project("Demo")
        editable = self._editable_track(project)
        first = add_editable_marker(project, editable.id, 0.25, "First")
        second = add_editable_marker(project, editable.id, 1.25, "Second")

        with self.assertRaisesRegex(ValueError, "negative timestamp"):
            move_editable_markers(project, editable.id, [first.id, second.id], -0.5)

        self.assertEqual(first.timestamp, 0.25)
        self.assertEqual(second.timestamp, 1.25)

    def test_resize_editable_marker_sets_duration_and_rejects_negative_duration(self):
        project = new_project("Demo")
        editable = self._editable_track(project)
        marker = add_editable_marker(project, editable.id, 0.25, "Cue")

        resize_editable_marker(project, editable.id, marker.id, 1.5)
        self.assertEqual(marker.duration, 1.5)

        with self.assertRaisesRegex(ValueError, "duration"):
            resize_editable_marker(project, editable.id, marker.id, -0.1)
        self.assertEqual(marker.duration, 1.5)

    def test_marker_editing_service_snaps_to_visible_timing_markers(self):
        project = new_project("Demo")
        source = self._source_track(project)
        timing = add_generated_track(
            project,
            source.id,
            "Beat Markers",
            "timing.beats",
            {},
            "1",
            "markers.v1",
            "dep",
        )
        timing.result_state = ResultState.COMPLETE
        project.markers.append(Marker(id="beat_1", track_id=timing.id, timestamp=1.0, category="timing"))
        service = MarkerEditingService()

        snapped = service.snap_time(
            project,
            requested_seconds=1.03,
            pixels_per_second=100.0,
            visible_track_ids=[timing.id],
            bypass=False,
        )

        self.assertEqual(snapped, 1.0)
        self.assertEqual(
            service.snap_time(
                project,
                requested_seconds=1.03,
                pixels_per_second=100.0,
                visible_track_ids=[timing.id],
                bypass=True,
            ),
            1.03,
        )
```

Add helper `_source_track()` to `EditableMarkerInspectorTest`:

```python
    def _source_track(self, project):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            return import_audio_asset(project, audio_path)
```

- [x] **Step 2: Run marker-editing tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_editable_marker_inspector.EditableMarkerInspectorTest.test_create_manual_editable_track_uses_resolved_source_track tests.test_editable_marker_inspector.EditableMarkerInspectorTest.test_create_manual_editable_track_rejects_track_without_source_context tests.test_editable_marker_inspector.EditableMarkerInspectorTest.test_move_editable_markers_is_atomic_for_negative_result tests.test_editable_marker_inspector.EditableMarkerInspectorTest.test_resize_editable_marker_sets_duration_and_rejects_negative_duration tests.test_editable_marker_inspector.EditableMarkerInspectorTest.test_marker_editing_service_snaps_to_visible_timing_markers -v
```

Expected: FAIL with missing helper imports or methods.

- [x] **Step 3: Implement manual-track and marker-editing store helpers**

Add to `autolight/project/store.py` near editable marker helpers:

```python
def create_manual_editable_track(project: ProjectDocument, context_track_id: str, name: str) -> Track:
    source_track = _source_track_for_context(project, context_track_id)
    if source_track is None:
        raise ValueError("manual cue tracks require a source audio context")
    track = Track(
        id=new_id("track"),
        type=TrackType.EDITABLE,
        name=str(name) or "Manual Cues",
        input_track_ids=[source_track.id],
        result_state=ResultState.COMPLETE,
        provenance={
            "source_track_id": source_track.id,
            "manual_track": True,
            "created_by": "user",
        },
    )
    project.tracks.append(track)
    return track


def move_editable_markers(
    project: ProjectDocument,
    track_id: str,
    marker_ids: list[str],
    delta_seconds: float,
) -> list[Marker]:
    _editable_track_or_raise(project, track_id)
    selected = _editable_markers_or_raise(project, track_id, marker_ids)
    delta = _finite_marker_delta(delta_seconds)
    next_timestamps = [marker.timestamp + delta for marker in selected]
    if any(timestamp < 0.0 for timestamp in next_timestamps):
        raise ValueError("marker move would create a negative timestamp")
    for marker, timestamp in zip(selected, next_timestamps, strict=True):
        marker.timestamp = timestamp
    if selected:
        mark_dependents_stale(project, track_id)
    return selected


def resize_editable_marker(
    project: ProjectDocument,
    track_id: str,
    marker_id: str,
    duration: float,
) -> Marker:
    _editable_track_or_raise(project, track_id)
    marker = _editable_marker_or_raise(project, track_id, marker_id)
    duration_value = _finite_marker_duration(duration)
    if marker.duration == duration_value:
        return marker
    marker.duration = duration_value
    mark_dependents_stale(project, track_id)
    return marker


def marker_snapshot(marker: Marker) -> dict[str, Any]:
    return {
        "id": marker.id,
        "track_id": marker.track_id,
        "timestamp": marker.timestamp,
        "duration": marker.duration,
        "label": marker.label,
        "category": marker.category,
        "confidence": marker.confidence,
        "tags": list(marker.tags),
        "source_transform": marker.source_transform,
        "source_marker_ids": list(marker.source_marker_ids),
        "metadata": dict(marker.metadata) if isinstance(marker.metadata, dict) else {},
    }
```

Add helper functions:

```python
def _source_track_for_context(project: ProjectDocument, track_id: str) -> Track | None:
    track = find_track(project, track_id)
    if track is None:
        return None
    visited: set[str] = set()
    stack = [track]
    while stack:
        current = stack.pop()
        if current.id in visited:
            continue
        visited.add(current.id)
        if current.type == TrackType.SOURCE:
            return current
        for input_id in current.input_track_ids:
            parent = find_track(project, input_id)
            if parent is not None:
                stack.append(parent)
    return None


def _editable_markers_or_raise(project: ProjectDocument, track_id: str, marker_ids: list[str]) -> list[Marker]:
    markers_by_id = {
        marker.id: marker
        for marker in project.markers
        if marker.track_id == track_id
    }
    markers = []
    for marker_id in marker_ids:
        try:
            markers.append(markers_by_id[marker_id])
        except KeyError as exc:
            raise ValueError(f"marker not found on track {track_id}: {marker_id}") from exc
    return markers


def _finite_marker_delta(value: float) -> float:
    number = float(value)
    if not math.isfinite(number):
        raise ValueError("marker delta must be finite")
    return number


def _finite_marker_duration(value: float) -> float:
    number = float(value)
    if not math.isfinite(number):
        raise ValueError("marker duration must be finite")
    if number < 0.0:
        raise ValueError("marker duration must be greater than or equal to zero")
    return number
```

Export new helpers from `autolight/project/__init__.py`.

- [x] **Step 4: Implement snapping service**

Replace `autolight/app/marker_editing.py` with:

```python
from __future__ import annotations

import math

from autolight.project.models import ProjectDocument, ResultState


SNAP_THRESHOLD_PIXELS = 8.0


class MarkerEditingService:
    def snap_time(
        self,
        project: ProjectDocument,
        *,
        requested_seconds: float,
        pixels_per_second: float,
        visible_track_ids: list[str],
        bypass: bool,
    ) -> float:
        requested = self._finite_non_negative(requested_seconds)
        if bypass:
            return requested
        zoom = max(1.0, self._finite_positive(pixels_per_second, fallback=96.0))
        threshold_seconds = SNAP_THRESHOLD_PIXELS / zoom
        visible = set(visible_track_ids)
        candidates = [
            marker.timestamp
            for marker in project.markers
            if marker.track_id in visible
            and self._track_can_snap(project, marker.track_id)
        ]
        if not candidates:
            return requested
        best = min(candidates, key=lambda value: abs(value - requested))
        return best if abs(best - requested) <= threshold_seconds else requested

    def _track_can_snap(self, project: ProjectDocument, track_id: str) -> bool:
        track = next((item for item in project.tracks if item.id == track_id), None)
        if track is None:
            return False
        return track.type.value == "generated" and track.result_state in {
            ResultState.COMPLETE,
            ResultState.STALE,
        }

    @staticmethod
    def _finite_non_negative(value: float) -> float:
        number = float(value)
        return number if math.isfinite(number) and number >= 0.0 else 0.0

    @staticmethod
    def _finite_positive(value: float, *, fallback: float) -> float:
        number = float(value)
        return number if math.isfinite(number) and number > 0.0 else fallback
```

- [x] **Step 5: Run marker-editing tests**

Run:

```bash
uv run python -m unittest tests.test_editable_marker_inspector -v
```

Expected: PASS.

- [x] **Step 6: Commit marker-editing domain helpers**

```bash
git add autolight/project/store.py autolight/project/__init__.py autolight/app/marker_editing.py tests/test_editable_marker_inspector.py
git commit -m "Add manual cue track and marker edit helpers"
```

Expected: commit succeeds.

## Task 4: Undo And Redo Command Stack

**Files:**
- Modify: `autolight/app/edit_history.py`
- Modify: `autolight/app_controller.py`
- Modify: `tests/test_editable_marker_inspector.py`
- Modify: `tests/test_app_controller.py`

- [x] **Step 1: Add failing edit-history tests**

Add these imports to `tests/test_editable_marker_inspector.py`:

```python
from autolight.app.edit_history import EditHistory, MarkerSnapshotCommand
from autolight.project.store import marker_snapshot
```

Add tests:

```python
    def test_edit_history_undoes_and_redoes_marker_snapshot_command(self):
        project = new_project("Demo")
        editable = self._editable_track(project)
        marker = add_editable_marker(project, editable.id, 1.0, "Cue", color="cyan")
        before = [marker_snapshot(marker)]
        update_editable_marker(
            project,
            editable.id,
            marker.id,
            timestamp=2.0,
            label="Hit",
            category="accent",
            color="amber",
        )
        after = [marker_snapshot(marker)]
        history = EditHistory()
        history.push(MarkerSnapshotCommand(track_id=editable.id, before=before, after=after))

        self.assertTrue(history.can_undo)
        history.undo(project)
        marker = next(item for item in project.markers if item.id == marker.id)
        self.assertEqual(marker.timestamp, 1.0)
        self.assertEqual(marker.label, "Cue")
        self.assertEqual(marker.metadata["color"], "cyan")

        self.assertTrue(history.can_redo)
        history.redo(project)
        marker = next(item for item in project.markers if item.id == marker.id)
        self.assertEqual(marker.timestamp, 2.0)
        self.assertEqual(marker.label, "Hit")
        self.assertEqual(marker.metadata["color"], "amber")
```

Add to `tests/test_app_controller.py`:

```python
    def test_controller_exposes_undo_redo_state_and_clears_history_on_new_project(self):
        controller = self._controller()

        self.assertFalse(controller.canUndo)
        self.assertFalse(controller.canRedo)
        controller.load_demo_project()
        editable_id = next(track.id for track in controller._project.tracks if track.type == TrackType.EDITABLE)
        controller.select_track(editable_id)
        marker_id = controller.add_marker_to_selected_track(0.75, "Cue", "cue", "cyan")

        self.assertTrue(controller.canUndo)
        self.assertFalse(controller.canRedo)
        self.assertTrue(controller.undo())
        self.assertFalse(any(marker.id == marker_id for marker in controller._project.markers))
        self.assertTrue(controller.canRedo)

        controller.new_project()
        self.assertFalse(controller.canUndo)
        self.assertFalse(controller.canRedo)
```

- [x] **Step 2: Run edit-history tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_editable_marker_inspector.EditableMarkerInspectorTest.test_edit_history_undoes_and_redoes_marker_snapshot_command tests.test_app_controller.AppControllerTest.test_controller_exposes_undo_redo_state_and_clears_history_on_new_project -v
```

Expected: FAIL with missing command classes and controller properties.

- [x] **Step 3: Implement edit-history commands**

Replace `autolight/app/edit_history.py` with:

```python
from __future__ import annotations

import copy
from dataclasses import dataclass
from typing import Protocol

from autolight.project.models import Marker, ProjectDocument
from autolight.project.store import find_track, mark_dependents_stale


class EditCommand(Protocol):
    def undo(self, project: ProjectDocument) -> None: ...
    def redo(self, project: ProjectDocument) -> None: ...


@dataclass(slots=True)
class MarkerSnapshotCommand:
    track_id: str
    before: list[dict]
    after: list[dict]

    def undo(self, project: ProjectDocument) -> None:
        self._restore(project, self.before)

    def redo(self, project: ProjectDocument) -> None:
        self._restore(project, self.after)

    def _restore(self, project: ProjectDocument, snapshots: list[dict]) -> None:
        affected_ids = {item["id"] for item in self.before} | {item["id"] for item in self.after}
        project.markers[:] = [
            marker
            for marker in project.markers
            if not (marker.track_id == self.track_id and marker.id in affected_ids)
        ]
        for item in snapshots:
            project.markers.append(
                Marker(
                    id=item["id"],
                    track_id=item["track_id"],
                    timestamp=item["timestamp"],
                    duration=item["duration"],
                    label=item["label"],
                    category=item["category"],
                    confidence=item["confidence"],
                    tags=list(item["tags"]),
                    source_transform=item["source_transform"],
                    source_marker_ids=list(item["source_marker_ids"]),
                    metadata=dict(item["metadata"]),
                )
            )
        if find_track(project, self.track_id) is not None:
            mark_dependents_stale(project, self.track_id)


@dataclass(slots=True)
class ProjectSnapshotCommand:
    before: ProjectDocument
    after: ProjectDocument

    def undo(self, project: ProjectDocument) -> None:
        self._restore(project, self.before)

    def redo(self, project: ProjectDocument) -> None:
        self._restore(project, self.after)

    def _restore(self, project: ProjectDocument, snapshot: ProjectDocument) -> None:
        project.id = snapshot.id
        project.name = snapshot.name
        project.schema_version = snapshot.schema_version
        project.audio_assets[:] = copy.deepcopy(snapshot.audio_assets)
        project.tracks[:] = copy.deepcopy(snapshot.tracks)
        project.markers[:] = copy.deepcopy(snapshot.markers)
        project.job_runs[:] = copy.deepcopy(snapshot.job_runs)
        project.cache_entries[:] = copy.deepcopy(snapshot.cache_entries)
        project.ui_state.clear()
        project.ui_state.update(copy.deepcopy(snapshot.ui_state))


class EditHistory:
    def __init__(self):
        self._undo_stack: list[EditCommand] = []
        self._redo_stack: list[EditCommand] = []

    @property
    def can_undo(self) -> bool:
        return bool(self._undo_stack)

    @property
    def can_redo(self) -> bool:
        return bool(self._redo_stack)

    def push(self, command: EditCommand) -> None:
        self._undo_stack.append(command)
        self._redo_stack.clear()

    def undo(self, project: ProjectDocument) -> bool:
        if not self._undo_stack:
            return False
        command = self._undo_stack.pop()
        command.undo(project)
        self._redo_stack.append(command)
        return True

    def redo(self, project: ProjectDocument) -> bool:
        if not self._redo_stack:
            return False
        command = self._redo_stack.pop()
        command.redo(project)
        self._undo_stack.append(command)
        return True

    def clear(self) -> None:
        self._undo_stack.clear()
        self._redo_stack.clear()
```

- [x] **Step 4: Integrate controller undo/redo state**

Add signals to `AppController`:

```python
    canUndoChanged = Signal()
    canRedoChanged = Signal()
```

Add properties:

```python
    @Property(bool, notify=canUndoChanged)
    def canUndo(self) -> bool:
        return self._edit_history.can_undo

    @Property(bool, notify=canRedoChanged)
    def canRedo(self) -> bool:
        return self._edit_history.can_redo
```

Add slots:

```python
    @Slot(result=bool)
    def undo(self) -> bool:
        try:
            if not self._edit_history.undo(self._project):
                return False
            self.trackModel.set_project(self._project)
            self.selectedTrackMarkersChanged.emit()
            self._notify_history_changed()
            self._set_dirty(True)
            return True
        except Exception as exc:
            self._set_last_error(str(exc))
            return False

    @Slot(result=bool)
    def redo(self) -> bool:
        try:
            if not self._edit_history.redo(self._project):
                return False
            self.trackModel.set_project(self._project)
            self.selectedTrackMarkersChanged.emit()
            self._notify_history_changed()
            self._set_dirty(True)
            return True
        except Exception as exc:
            self._set_last_error(str(exc))
            return False

    def _notify_history_changed(self) -> None:
        self.canUndoChanged.emit()
        self.canRedoChanged.emit()
```

Call `self._edit_history.clear()` and `_notify_history_changed()` from `_set_project()`.

Wrap existing marker mutation slots with history commands. For `add_marker_to_selected_track()`, capture `after = [marker_snapshot(marker)]` and push `MarkerSnapshotCommand(track_id=self._selected_track_id, before=[], after=after)` after mutation succeeds.

Add a full-project snapshot helper for track creation and any edit that mutates track lists:

```python
    def _push_project_snapshot_command(self, before_project) -> None:
        self._edit_history.push(
            ProjectSnapshotCommand(
                before=before_project,
                after=copy.deepcopy(self._project),
            )
        )
        self._notify_history_changed()
```

Import `copy` and `ProjectSnapshotCommand` in `autolight/app_controller.py`.

- [x] **Step 5: Run edit-history tests**

Run:

```bash
uv run python -m unittest tests.test_editable_marker_inspector tests.test_app_controller.AppControllerTest.test_controller_exposes_undo_redo_state_and_clears_history_on_new_project -v
```

Expected: PASS.

- [x] **Step 6: Commit edit history**

```bash
git add autolight/app/edit_history.py autolight/app_controller.py tests/test_editable_marker_inspector.py tests/test_app_controller.py
git commit -m "Add undo redo stack for timeline edits"
```

Expected: commit succeeds.

## Task 5: Controller Slots For Manual Tracks, Move, Resize, And Snap

**Files:**
- Modify: `autolight/app_controller.py`
- Modify: `tests/test_app_controller.py`

- [x] **Step 1: Add failing controller slot tests**

Add tests:

```python
    def test_controller_adds_manual_cue_track_from_selected_source_context(self):
        controller = self._controller()
        controller.load_demo_project()
        source = next(track for track in controller._project.tracks if track.type == TrackType.SOURCE)
        controller.select_track(source.id)

        manual_id = controller.add_manual_cue_track("Manual Cues")

        self.assertNotEqual(manual_id, "")
        manual = self._track_by_id(controller, manual_id)
        self.assertEqual(manual.type, TrackType.EDITABLE)
        self.assertEqual(manual.input_track_ids, [source.id])
        self.assertEqual(controller.selectedTrackId, manual_id)
        self.assertTrue(controller.canUndo)

    def test_controller_moves_and_resizes_selected_editable_markers(self):
        controller = self._controller()
        controller.load_demo_project()
        editable = next(track for track in controller._project.tracks if track.type == TrackType.EDITABLE)
        controller.select_track(editable.id)
        marker_id = controller.add_marker_to_selected_track(0.5, "Cue", "cue", "cyan")
        controller.toggle_marker_selection(marker_id, False)

        self.assertTrue(controller.move_selected_markers(0.25, False))
        marker = next(marker for marker in controller._project.markers if marker.id == marker_id)
        self.assertEqual(marker.timestamp, 0.75)

        self.assertTrue(controller.resize_marker(marker_id, 1.25))
        self.assertEqual(marker.duration, 1.25)

    def test_controller_snap_time_uses_generated_timing_markers_and_bypass(self):
        controller = self._controller()
        controller.load_demo_project()
        timing = next(track for track in controller._project.tracks if track.name == "Beat Markers")

        self.assertEqual(controller.snap_timeline_time(0.53, False), 0.5)
        self.assertEqual(controller.snap_timeline_time(0.53, True), 0.53)
        controller.select_track(timing.id)
        self.assertEqual(controller.snap_timeline_time(1.03, False), 1.0)
```

- [x] **Step 2: Run controller slot tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_controller_adds_manual_cue_track_from_selected_source_context tests.test_app_controller.AppControllerTest.test_controller_moves_and_resizes_selected_editable_markers tests.test_app_controller.AppControllerTest.test_controller_snap_time_uses_generated_timing_markers_and_bypass -v
```

Expected: FAIL because slots are missing.

- [x] **Step 3: Implement controller slots**

Add imports:

```python
import copy

from autolight.project.store import (
    create_manual_editable_track,
    marker_snapshot,
    move_editable_markers,
    resize_editable_marker,
)
from autolight.app.edit_history import MarkerSnapshotCommand, ProjectSnapshotCommand
```

Add slots:

```python
    @Slot(str, result=str)
    def add_manual_cue_track(self, name: str = "Manual Cues") -> str:
        try:
            before_project = copy.deepcopy(self._project)
            track = create_manual_editable_track(self._project, self._selected_track_id, name or "Manual Cues")
            self.trackModel.set_project(self._project)
            self._set_selected_track_id(track.id)
            self._set_dirty(True)
            self._notify_timeline_duration_changed()
            self._push_project_snapshot_command(before_project)
            return track.id
        except Exception as exc:
            self._set_last_error(str(exc))
            return ""

    @Slot(float, bool, result=bool)
    def move_selected_markers(self, delta_seconds: float, bypass_snap: bool = False) -> bool:
        try:
            if not self._selected_marker_ids:
                raise ValueError("select at least one marker to move")
            before = [marker_snapshot(self._editable_marker_for_selected_marker_id(marker_id)) for marker_id in self._selected_marker_ids]
            delta = float(delta_seconds)
            if not bypass_snap and len(self._selected_marker_ids) == 1:
                marker = self._editable_marker_for_selected_marker_id(self._selected_marker_ids[0])
                snapped = self.snap_timeline_time(marker.timestamp + delta, False)
                delta = snapped - marker.timestamp
            moved = move_editable_markers(self._project, self._selected_track_id, self._selected_marker_ids, delta)
            after = [marker_snapshot(marker) for marker in moved]
            self._edit_history.push(MarkerSnapshotCommand(track_id=self._selected_track_id, before=before, after=after))
            self._notify_history_changed()
            self.trackModel.refresh_track(self._selected_track_id)
            self.selectedTrackMarkersChanged.emit()
            self._notify_timeline_duration_changed()
            self._set_dirty(True)
            return True
        except Exception as exc:
            self._set_last_error(str(exc))
            return False

    @Slot(str, float, result=bool)
    def resize_marker(self, marker_id: str, duration: float) -> bool:
        try:
            marker = self._editable_marker_for_selected_marker_id(marker_id)
            before = [marker_snapshot(marker)]
            updated = resize_editable_marker(self._project, self._selected_track_id, marker_id, duration)
            after = [marker_snapshot(updated)]
            self._edit_history.push(MarkerSnapshotCommand(track_id=self._selected_track_id, before=before, after=after))
            self._notify_history_changed()
            self.trackModel.refresh_track(self._selected_track_id)
            self.selectedTrackMarkersChanged.emit()
            self._notify_timeline_duration_changed()
            self._set_dirty(True)
            return True
        except Exception as exc:
            self._set_last_error(str(exc))
            return False

    @Slot(float, bool, result=float)
    def snap_timeline_time(self, seconds: float, bypass_snap: bool = False) -> float:
        return self._marker_editing.snap_time(
            self._project,
            requested_seconds=seconds,
            pixels_per_second=self._timeline_pixels_per_second,
            visible_track_ids=[track.id for track in self._project.tracks],
            bypass=bypass_snap,
        )
```

Use the `_push_project_snapshot_command()` helper from Task 4 for manual track creation so undo removes the newly-created track and redo restores it.

- [x] **Step 4: Run controller slot tests**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_controller_adds_manual_cue_track_from_selected_source_context tests.test_app_controller.AppControllerTest.test_controller_moves_and_resizes_selected_editable_markers tests.test_app_controller.AppControllerTest.test_controller_snap_time_uses_generated_timing_markers_and_bypass -v
```

Expected: PASS.

- [x] **Step 5: Run controller and editable marker suites**

Run:

```bash
uv run python -m unittest tests.test_app_controller tests.test_editable_marker_inspector -v
```

Expected: PASS.

- [x] **Step 6: Commit controller editing slots**

```bash
git add autolight/app_controller.py autolight/app/edit_history.py tests/test_app_controller.py
git commit -m "Expose direct marker editing controller slots"
```

Expected: commit succeeds.

## Task 6: Waveform Pyramid Generation And LOD Selection

**Files:**
- Modify: `autolight/analysis/waveform.py`
- Modify: `autolight/analysis/builtin.py`
- Modify: `autolight/app/waveform_lod.py`
- Modify: `autolight/app_controller.py`
- Modify: `tests/test_waveform_summary.py`

- [x] **Step 1: Add failing waveform LOD tests**

Add imports:

```python
from autolight.app.waveform_lod import WaveformLodStore
```

Add tests to `WaveformSummaryTest`:

```python
    def test_build_waveform_summary_writes_pyramid_levels(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            output_path = Path(tmp) / "waveform.json"
            write_wav(audio_path)

            build_waveform_summary(audio_path, output_path, buckets=2)
            payload = json.loads(output_path.read_text(encoding="utf-8"))

        self.assertEqual(payload["version"], 2)
        self.assertIn("levels", payload)
        self.assertGreaterEqual(len(payload["levels"]), 2)
        self.assertEqual(payload["levels"][0]["bucket_count"], 2)
        self.assertGreater(payload["levels"][-1]["bucket_count"], payload["levels"][0]["bucket_count"])

    def test_waveform_lod_selects_more_detail_when_zoomed_in(self):
        payload = {
            "version": 2,
            "duration": 8.0,
            "levels": [
                {"bucket_count": 8, "samples": [{"peak": 0.1, "rms": 0.05}] * 8},
                {"bucket_count": 64, "samples": [{"peak": 0.2, "rms": 0.10}] * 64},
            ],
        }
        store = WaveformLodStore()

        overview = store.visible_samples(payload, scroll_seconds=0.0, visible_seconds=8.0, pixels_per_second=48.0)
        detail = store.visible_samples(payload, scroll_seconds=0.0, visible_seconds=1.0, pixels_per_second=200.0)

        self.assertEqual(overview["level_bucket_count"], 8)
        self.assertEqual(detail["level_bucket_count"], 64)
        self.assertLessEqual(len(detail["samples"]), 16)

    def test_waveform_lod_reads_legacy_single_sample_payload(self):
        payload = {
            "version": 1,
            "duration": 1.0,
            "samples": [{"peak": 0.25, "rms": 0.10}],
        }
        store = WaveformLodStore()

        visible = store.visible_samples(payload, scroll_seconds=0.0, visible_seconds=1.0, pixels_per_second=96.0)

        self.assertEqual(visible["level_bucket_count"], 1)
        self.assertEqual(visible["samples"][0]["peak"], 0.25)
```

- [x] **Step 2: Run waveform LOD tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_waveform_summary.WaveformSummaryTest.test_build_waveform_summary_writes_pyramid_levels tests.test_waveform_summary.WaveformSummaryTest.test_waveform_lod_selects_more_detail_when_zoomed_in tests.test_waveform_summary.WaveformSummaryTest.test_waveform_lod_reads_legacy_single_sample_payload -v
```

Expected: FAIL because waveform payloads are version 1 and `WaveformLodStore` is empty.

- [x] **Step 3: Implement waveform pyramid payloads**

Add this module constant near `WAVEFORM_READ_BLOCK_FRAMES`:

```python
MAX_WAVEFORM_LOD_BUCKETS = 4_096
```

Update `build_waveform_summary()` in `autolight/analysis/waveform.py` so it writes both legacy-compatible `samples` and new `levels`:

```python
    base_bucket_count = min(buckets, max(1, frame_count))
    level_bucket_counts = _waveform_level_bucket_counts(base_bucket_count, frame_count)
    levels = []
    for bucket_count in level_bucket_counts:
        levels.append(
            {
                "bucket_count": bucket_count,
                "samples": _summarize_samples(audio_path, bucket_count, cancel_requested),
            }
        )
    payload = {
        "version": 2,
        "sample_rate": sample_rate,
        "duration": 0.0 if sample_rate == 0 else float(frame_count / sample_rate),
        "samples": levels[0]["samples"],
        "levels": levels,
    }
```

Implement helper:

```python
def _waveform_level_bucket_counts(base_bucket_count: int, frame_count: int) -> list[int]:
    maximum = min(MAX_WAVEFORM_LOD_BUCKETS, max(1, frame_count))
    counts = [max(1, min(base_bucket_count, maximum))]
    while counts[-1] < maximum:
        next_count = min(maximum, counts[-1] * 4)
        if next_count == counts[-1]:
            break
        counts.append(next_count)
    return counts
```

Keep existing soundfile/audioread summarization paths by extracting the old single-resolution summary into `_summarize_samples(audio_path, bucket_count, cancel_requested)`.

- [x] **Step 4: Implement LOD parsing and visible slicing**

Replace `autolight/app/waveform_lod.py`:

```python
from __future__ import annotations

import math
from typing import Any


TARGET_PIXELS_PER_BUCKET = 4.0


class WaveformLodStore:
    def visible_samples(
        self,
        payload: dict[str, Any],
        *,
        scroll_seconds: float,
        visible_seconds: float,
        pixels_per_second: float,
    ) -> dict[str, Any]:
        duration = self._duration(payload)
        levels = self._levels(payload)
        if not levels:
            return {"duration": duration, "level_bucket_count": 0, "samples": []}
        level = self._select_level(levels, visible_seconds=visible_seconds, pixels_per_second=pixels_per_second)
        bucket_count = int(level["bucket_count"])
        samples = list(level["samples"])
        if duration <= 0.0 or bucket_count <= 0:
            return {"duration": duration, "level_bucket_count": bucket_count, "samples": []}
        start_seconds = max(0.0, float(scroll_seconds))
        stop_seconds = min(duration, start_seconds + max(0.01, float(visible_seconds)))
        start_index = max(0, math.floor(start_seconds / duration * bucket_count) - 1)
        stop_index = min(bucket_count, math.ceil(stop_seconds / duration * bucket_count) + 1)
        visible = [
            {**sample, "time": (index / max(1, bucket_count - 1)) * duration}
            for index, sample in enumerate(samples[start_index:stop_index], start=start_index)
        ]
        return {
            "duration": duration,
            "level_bucket_count": bucket_count,
            "samples": visible,
        }

    def _select_level(self, levels: list[dict[str, Any]], *, visible_seconds: float, pixels_per_second: float) -> dict[str, Any]:
        visible = max(0.01, float(visible_seconds))
        zoom = max(1.0, float(pixels_per_second))
        desired = max(1, math.ceil(visible * zoom / TARGET_PIXELS_PER_BUCKET))
        return min(levels, key=lambda level: abs(int(level["bucket_count"]) - desired))

    def _levels(self, payload: dict[str, Any]) -> list[dict[str, Any]]:
        levels = payload.get("levels")
        if isinstance(levels, list) and levels:
            return [
                {
                    "bucket_count": int(level.get("bucket_count", len(level.get("samples", [])))),
                    "samples": list(level.get("samples", [])),
                }
                for level in levels
            ]
        samples = payload.get("samples", [])
        return [{"bucket_count": len(samples), "samples": list(samples)}] if isinstance(samples, list) else []

    def _duration(self, payload: dict[str, Any]) -> float:
        try:
            duration = float(payload.get("duration", 0.0))
        except (TypeError, ValueError):
            return 0.0
        return duration if math.isfinite(duration) and duration >= 0.0 else 0.0
```

- [x] **Step 5: Run waveform tests**

Run:

```bash
uv run python -m unittest tests.test_waveform_summary -v
```

Expected: PASS.

- [x] **Step 6: Commit waveform LOD data path**

```bash
git add autolight/analysis/waveform.py autolight/analysis/builtin.py autolight/app/waveform_lod.py tests/test_waveform_summary.py
git commit -m "Add zoom adaptive waveform LOD"
```

Expected: commit succeeds.

## Task 7: Timeline Model Roles For Editing And Visible Waveforms

**Files:**
- Modify: `autolight/timeline/model.py`
- Modify: `autolight/app_controller.py`
- Modify: `tests/test_timeline_model.py`
- Modify: `tests/test_waveform_summary.py`

- [x] **Step 1: Add failing timeline role tests**

Add tests to `tests/test_timeline_model.py`:

```python
    def test_model_exposes_editability_and_marker_duration(self):
        project = new_project("Demo")
        editable = Track(id="track_edit", type=TrackType.EDITABLE, name="Editable", result_state=ResultState.COMPLETE)
        project.tracks.append(editable)
        project.markers.append(Marker(id="marker_1", track_id=editable.id, timestamp=1.0, duration=0.5))
        model = TimelineTrackModel()
        model.set_project(project)

        index = model.index(0, 0)
        self.assertTrue(model.data(index, model.role_for_name("editable")))
        spans = model.data(index, model.role_for_name("markerSpans"))
        self.assertEqual(spans[0]["duration"], 0.5)
        self.assertFalse(spans[0]["selected"])

    def test_model_exposes_visible_waveform_samples(self):
        project = new_project("Demo")
        waveform = Track(
            id="track_wave",
            type=TrackType.GENERATED,
            name="Waveform",
            transform_id="waveform.summary",
            result_state=ResultState.COMPLETE,
            provenance={
                "visible_waveform": {
                    "duration": 2.0,
                    "level_bucket_count": 8,
                    "samples": [{"time": 0.0, "peak": 0.2, "rms": 0.1}],
                }
            },
        )
        project.tracks.append(waveform)
        project.cache_entries.append(CacheEntry(id="cache_1", dependency_hash="dep", artifact_kind="waveform", path="waveform.json", created_at="", transform_version="1"))
        waveform.cache_refs = ["cache_1"]
        model = TimelineTrackModel()
        model.set_project(project)

        index = model.index(0, 0)
        visible = model.data(index, model.role_for_name("visibleWaveformSamples"))
        self.assertEqual(visible[0]["time"], 0.0)
        self.assertEqual(model.data(index, model.role_for_name("waveformLevelBucketCount")), 8)
```

- [x] **Step 2: Run timeline role tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_timeline_model.TimelineTrackModelTest.test_model_exposes_editability_and_marker_duration tests.test_timeline_model.TimelineTrackModelTest.test_model_exposes_visible_waveform_samples -v
```

Expected: FAIL because roles are missing.

- [x] **Step 3: Add timeline roles**

Add roles to `TimelineTrackModel.ROLE_NAMES`:

```python
        Qt.ItemDataRole.UserRole + 15: b"editable",
        Qt.ItemDataRole.UserRole + 16: b"visibleWaveformSamples",
        Qt.ItemDataRole.UserRole + 17: b"waveformLevelBucketCount",
```

Add handlers:

```python
            self.role_for_name("editable"): lambda track: track.type == TrackType.EDITABLE,
            self.role_for_name("visibleWaveformSamples"): self._visible_waveform_samples_for_track,
            self.role_for_name("waveformLevelBucketCount"): self._waveform_level_bucket_count_for_track,
```

Import `TrackType`.

Update `_marker_span()`:

```python
            "selected": bool(marker.metadata.get("selected", False)) if isinstance(marker.metadata, dict) else False,
```

Add waveform helpers:

```python
    def _visible_waveform_samples_for_track(self, track: Track) -> list:
        visible = track.provenance.get("visible_waveform", {})
        if not isinstance(visible, dict):
            return []
        samples = visible.get("samples", [])
        return samples if isinstance(samples, list) else []

    def _waveform_level_bucket_count_for_track(self, track: Track) -> int:
        visible = track.provenance.get("visible_waveform", {})
        if not isinstance(visible, dict):
            return 0
        try:
            return int(visible.get("level_bucket_count", 0))
        except (TypeError, ValueError):
        return 0
```

Update existing `tests/test_timeline_model.py` role-name expectations to include:

```python
                    model.role_for_name("editable"): b"editable",
                    model.role_for_name("visibleWaveformSamples"): b"visibleWaveformSamples",
                    model.role_for_name("waveformLevelBucketCount"): b"waveformLevelBucketCount",
```

Update existing expected marker-span dictionaries to include:

```python
                        "selected": False,
```

- [x] **Step 4: Integrate controller visible waveform refresh**

In `AppController`, add helper:

```python
    def _refresh_visible_waveforms(self) -> None:
        for track in self._project.tracks:
            if track.transform_id != "waveform.summary":
                continue
            payload = track.provenance.get("waveform_payload")
            if not isinstance(payload, dict):
                continue
            track.provenance["visible_waveform"] = self._waveform_lod.visible_samples(
                payload,
                scroll_seconds=self._timeline_scroll_seconds,
                visible_seconds=self._visible_timeline_seconds(),
                pixels_per_second=self._timeline_pixels_per_second,
            )
            self.trackModel.refresh_track(track.id)
```

Call `_refresh_visible_waveforms()` after `_load_waveform_samples()`, after scroll changes, after zoom changes, and after visible seconds changes.

- [x] **Step 5: Run timeline and waveform focused tests**

Run:

```bash
uv run python -m unittest tests.test_timeline_model tests.test_waveform_summary -v
```

Expected: PASS.

- [x] **Step 6: Commit timeline waveform roles**

```bash
git add autolight/timeline/model.py autolight/app_controller.py tests/test_timeline_model.py tests/test_waveform_summary.py
git commit -m "Expose editable and waveform LOD timeline roles"
```

Expected: commit succeeds.

## Task 8: QML Component Extraction

**Files:**
- Create: `UI/components/ProjectToolbar.qml`
- Create: `UI/components/TransformBar.qml`
- Create: `UI/components/PlaybackBar.qml`
- Create: `UI/components/TimelineRuler.qml`
- Create: `UI/components/TimelineView.qml`
- Create: `UI/components/TrackRow.qml`
- Create: `UI/components/TimelineLane.qml`
- Create: `UI/components/MarkerBlock.qml`
- Create: `UI/components/WaveformStrip.qml`
- Create: `UI/components/MarkerInspector.qml`
- Create: `UI/components/StatusFooter.qml`
- Modify: `UI/Main.qml`
- Modify: `tests/test_app_controller.py`

- [x] **Step 1: Add failing QML componentization tests**

Add tests:

```python
    def test_qml_main_composes_milestone_2_components(self):
        qml = Path("UI/Main.qml").read_text(encoding="utf-8")

        for component_name in [
            "ProjectToolbar",
            "TransformBar",
            "PlaybackBar",
            "TimelineRuler",
            "TimelineView",
            "MarkerInspector",
            "StatusFooter",
        ]:
            with self.subTest(component_name=component_name):
                self.assertIn(component_name, qml)

    def test_qml_large_components_are_split_out_of_main(self):
        self.assertLessEqual(len(Path("UI/Main.qml").read_text(encoding="utf-8").splitlines()), 360)
        for path in [
            Path("UI/components/TimelineLane.qml"),
            Path("UI/components/MarkerBlock.qml"),
            Path("UI/components/WaveformStrip.qml"),
        ]:
            with self.subTest(path=str(path)):
                self.assertTrue(path.exists())
```

- [x] **Step 2: Run QML componentization tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_qml_main_composes_milestone_2_components tests.test_app_controller.AppControllerTest.test_qml_large_components_are_split_out_of_main -v
```

Expected: FAIL because components do not exist and `Main.qml` is still too large.

- [x] **Step 3: Extract root components without changing behavior**

Create `UI/components/ProjectToolbar.qml` with a `ToolBar` root and properties:

```qml
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ToolBar {
    id: root
    property var appController
    property int compactButtonHeight: 30
    signal newRequested()
    signal openRequested()
    signal saveAsRequested()
    signal demoRequested()
    signal importAudioRequested()

    RowLayout {
        anchors.fill: parent
        spacing: 8

        Label {
            text: root.appController.projectName
            font.pixelSize: 16
            font.bold: true
            Layout.leftMargin: 12
        }

        Item { Layout.fillWidth: true }

        Button { text: "New"; implicitHeight: root.compactButtonHeight; onClicked: root.newRequested() }
        Button { text: "Open"; implicitHeight: root.compactButtonHeight; onClicked: root.openRequested() }
        Button {
            text: "Save"
            implicitHeight: root.compactButtonHeight
            onClicked: root.appController.projectPath.length > 0 ? root.appController.save_project("") : root.saveAsRequested()
        }
        Button { text: "Save As"; implicitHeight: root.compactButtonHeight; onClicked: root.saveAsRequested() }
        Button { text: "Demo"; implicitHeight: root.compactButtonHeight; onClicked: root.demoRequested() }
        Button { text: "Import Audio"; implicitHeight: root.compactButtonHeight; onClicked: root.importAudioRequested() }
    }
}
```

Extract the existing transform controls, playback controls, ruler, timeline row/lane, marker inspector, and footer into the listed component files. Keep public signal names explicit rather than having components open dialogs directly. `Main.qml` should import components:

```qml
import "components"
```

`Main.qml` should wire component signals to existing root functions and dialogs.

- [x] **Step 4: Run QML componentization tests and smoke**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_qml_main_composes_milestone_2_components tests.test_app_controller.AppControllerTest.test_qml_large_components_are_split_out_of_main -v
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: PASS.

- [x] **Step 5: Commit QML component extraction**

```bash
git add UI/Main.qml UI/components tests/test_app_controller.py
git commit -m "Split QML shell into timeline components"
```

Expected: commit succeeds.

## Task 9: Direct Timeline Manipulation In QML

**Files:**
- Modify: `UI/components/TimelineLane.qml`
- Modify: `UI/components/MarkerBlock.qml`
- Modify: `UI/components/MarkerInspector.qml`
- Modify: `UI/components/ProjectToolbar.qml`
- Modify: `UI/Main.qml`
- Modify: `tests/test_app_controller.py`

- [x] **Step 1: Add failing direct-manipulation QML tests**

Add:

```python
    def test_qml_exposes_direct_marker_drag_resize_and_manual_tracks(self):
        lane_qml = Path("UI/components/TimelineLane.qml").read_text(encoding="utf-8")
        marker_qml = Path("UI/components/MarkerBlock.qml").read_text(encoding="utf-8")
        toolbar_qml = Path("UI/components/ProjectToolbar.qml").read_text(encoding="utf-8")

        self.assertIn("add_manual_cue_track", toolbar_qml)
        self.assertIn("snap_timeline_time", lane_qml)
        self.assertIn("move_selected_markers", marker_qml)
        self.assertIn("resize_marker", marker_qml)
        self.assertIn("AltModifier", marker_qml)

    def test_qml_exposes_undo_redo_actions(self):
        qml = Path("UI/Main.qml").read_text(encoding="utf-8")
        toolbar_qml = Path("UI/components/ProjectToolbar.qml").read_text(encoding="utf-8")

        self.assertIn("appController.undo()", toolbar_qml + qml)
        self.assertIn("appController.redo()", toolbar_qml + qml)
        self.assertIn("canUndo", toolbar_qml + qml)
        self.assertIn("canRedo", toolbar_qml + qml)
```

- [x] **Step 2: Run direct-manipulation QML tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_qml_exposes_direct_marker_drag_resize_and_manual_tracks tests.test_app_controller.AppControllerTest.test_qml_exposes_undo_redo_actions -v
```

Expected: FAIL because QML is not wired yet.

- [x] **Step 3: Add manual track and undo/redo actions**

In `ProjectToolbar.qml`, add buttons:

```qml
        Button {
            text: "Manual Track"
            implicitHeight: root.compactButtonHeight
            enabled: root.appController.selectedTrackId.length > 0
            onClicked: root.appController.add_manual_cue_track("Manual Cues")
        }
        Button {
            text: "Undo"
            implicitHeight: root.compactButtonHeight
            enabled: root.appController.canUndo
            onClicked: root.appController.undo()
        }
        Button {
            text: "Redo"
            implicitHeight: root.compactButtonHeight
            enabled: root.appController.canRedo
            onClicked: root.appController.redo()
        }
```

- [x] **Step 4: Add marker drag and resize handlers**

In `MarkerBlock.qml`, define properties and drag handling:

```qml
Rectangle {
    id: root
    property var appController
    property string markerId
    property real timestamp
    property real duration
    property bool editable: false
    property real pixelsPerSecond: 96
    property color markerColor: "#22d3ee"
    property string markerLabel: ""
    signal selected(string markerId, bool additive)

    color: root.markerColor
    radius: 2

    MouseArea {
        id: bodyDrag
        anchors.fill: parent
        enabled: root.editable
        drag.target: null
        property real pressX: 0
        property real lastPreviewDelta: 0

        onPressed: function(mouse) {
            pressX = mouse.x
            root.selected(root.markerId, (mouse.modifiers & Qt.ShiftModifier) !== 0)
        }

        onPositionChanged: function(mouse) {
            lastPreviewDelta = (mouse.x - pressX) / Math.max(1, root.pixelsPerSecond)
        }

        onReleased: function(mouse) {
            var bypass = (mouse.modifiers & Qt.AltModifier) !== 0
            var delta = (mouse.x - pressX) / Math.max(1, root.pixelsPerSecond)
            root.appController.move_selected_markers(delta, bypass)
            lastPreviewDelta = 0
        }
    }

    Rectangle {
        id: rightResizeHandle
        width: 8
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.right: parent.right
        color: "transparent"
        visible: root.editable

        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.SizeHorCursor
            property real startWidth: 0
            onPressed: startWidth = root.width
            onReleased: function(mouse) {
                var widthDelta = mouse.x
                var nextDuration = Math.max(0, (startWidth + widthDelta) / Math.max(1, root.pixelsPerSecond))
                root.appController.resize_marker(root.markerId, nextDuration)
            }
        }
    }
}
```

In `TimelineLane.qml`, instantiate `MarkerBlock` for `markerSpans`, pass `editable`, and route selection:

```qml
MarkerBlock {
    markerId: modelData.id
    timestamp: modelData.timestamp
    duration: modelData.duration
    markerColor: modelData.color
    markerLabel: modelData.label
    editable: lane.editable
    pixelsPerSecond: lane.appController.timelinePixelsPerSecond
    appController: lane.appController
    x: lane.timelineX(modelData.timestamp)
    width: Math.max(8, (modelData.duration > 0 ? modelData.duration : 0.08) * lane.appController.timelinePixelsPerSecond)
    height: parent.height - 18
    y: 9
    onSelected: function(markerId, additive) {
        lane.appController.toggle_marker_selection(markerId, additive)
    }
}
```

- [x] **Step 5: Run direct-manipulation tests and smoke**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_qml_exposes_direct_marker_drag_resize_and_manual_tracks tests.test_app_controller.AppControllerTest.test_qml_exposes_undo_redo_actions -v
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: PASS.

- [x] **Step 6: Commit direct manipulation QML**

```bash
git add UI/Main.qml UI/components tests/test_app_controller.py
git commit -m "Wire direct marker editing in QML"
```

Expected: commit succeeds.

## Task 10: Playback, Scrubbing, And Waveform Rendering Performance In QML

**Files:**
- Modify: `UI/components/PlaybackBar.qml`
- Modify: `UI/components/TimelineView.qml`
- Modify: `UI/components/TimelineLane.qml`
- Modify: `UI/components/WaveformStrip.qml`
- Modify: `tests/test_app_controller.py`

- [x] **Step 1: Add failing performance-structure QML tests**

Add:

```python
    def test_qml_waveform_uses_canvas_and_visible_samples(self):
        waveform_qml = Path("UI/components/WaveformStrip.qml").read_text(encoding="utf-8")

        self.assertIn("Canvas", waveform_qml)
        self.assertIn("visibleWaveformSamples", Path("UI/components/TimelineLane.qml").read_text(encoding="utf-8"))
        self.assertNotIn("model: waveformSamples", waveform_qml)

    def test_qml_scrubber_avoids_live_heavy_seek_binding(self):
        playback_qml = Path("UI/components/PlaybackBar.qml").read_text(encoding="utf-8")

        self.assertIn("onMoved", playback_qml)
        self.assertIn("onPressedChanged", playback_qml)
        self.assertIn("seek_playback", playback_qml)
```

- [x] **Step 2: Run performance-structure tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_qml_waveform_uses_canvas_and_visible_samples tests.test_app_controller.AppControllerTest.test_qml_scrubber_avoids_live_heavy_seek_binding -v
```

Expected: FAIL until waveform strip uses canvas and scrubber separates preview from committed seeks.

- [x] **Step 3: Implement batched waveform strip**

Create or replace `UI/components/WaveformStrip.qml`:

```qml
import QtQuick

Canvas {
    id: root
    property var samples: []
    property real durationSeconds: 0
    property real scrollSeconds: 0
    property real pixelsPerSecond: 96
    property real leftPadding: 24
    property color peakColor: "#60a5fa"
    property color rmsColor: "#bfdbfe"

    onSamplesChanged: requestPaint()
    onScrollSecondsChanged: requestPaint()
    onPixelsPerSecondChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()

    onPaint: {
        var ctx = getContext("2d")
        ctx.clearRect(0, 0, width, height)
        if (!samples || samples.length === 0) {
            return
        }
        var centerY = height / 2
        ctx.strokeStyle = rmsColor
        ctx.lineWidth = 1
        for (var i = 0; i < samples.length; i++) {
            var sample = samples[i]
            var x = leftPadding + (sample.time - scrollSeconds) * pixelsPerSecond
            if (x < leftPadding - 2 || x > width + 2) {
                continue
            }
            var peakHeight = Math.max(1, sample.peak * (height - 18))
            var rmsHeight = Math.max(1, sample.rms * (height - 18))
            ctx.strokeStyle = peakColor
            ctx.beginPath()
            ctx.moveTo(x, centerY - peakHeight / 2)
            ctx.lineTo(x, centerY + peakHeight / 2)
            ctx.stroke()
            ctx.strokeStyle = rmsColor
            ctx.beginPath()
            ctx.moveTo(x + 1, centerY - rmsHeight / 2)
            ctx.lineTo(x + 1, centerY + rmsHeight / 2)
            ctx.stroke()
        }
    }
}
```

Use `WaveformStrip` in `TimelineLane.qml`:

```qml
WaveformStrip {
    anchors.fill: parent
    samples: visibleWaveformSamples
    durationSeconds: waveformDurationSeconds
    scrollSeconds: lane.appController.timelineScrollSeconds
    pixelsPerSecond: lane.appController.timelinePixelsPerSecond
    leftPadding: lane.timelineLeftPadding
    visible: visibleWaveformSamples.length > 0
}
```

- [x] **Step 4: Reduce scrubber churn**

In `PlaybackBar.qml`, make the scrubber keep a local preview while pressed:

```qml
Slider {
    id: playbackScrubber
    property bool scrubbing: pressed
    property real previewValue: appController.playback.positionSeconds
    from: 0
    to: Math.max(0.01, appController.playback.durationSeconds)
    value: scrubbing ? previewValue : appController.playback.positionSeconds
    live: true
    enabled: appController.playback.sourcePath.length > 0
    onMoved: previewValue = value
    onPressedChanged: {
        if (!pressed) {
            appController.seek_playback(previewValue)
        } else {
            previewValue = value
        }
    }
}
```

- [x] **Step 5: Run performance-structure tests and smoke**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_qml_waveform_uses_canvas_and_visible_samples tests.test_app_controller.AppControllerTest.test_qml_scrubber_avoids_live_heavy_seek_binding -v
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: PASS.

- [x] **Step 6: Commit QML performance rendering**

```bash
git add UI/components tests/test_app_controller.py
git commit -m "Render waveform and scrubber through lighter QML paths"
```

Expected: commit succeeds.

## Task 11: Documentation, Full Verification, And Plan Closure

**Files:**
- Modify: `README.md`
- Modify: `docs/superpowers/plans/2026-06-01-autolight-interactive-timeline.md`

- [x] **Step 1: Update README workflow**

Update `README.md` `Current Scope` with:

```markdown
- Create blank manual cue tracks for direct authoring.
- Move, resize, select, and delete editable cue markers directly on the timeline.
- Undo and redo manual track and marker edits during the current app session.
- Snap editable marker movement to visible generated timing markers, with a modifier-key bypass for free placement.
- Render zoom-adaptive waveform detail while keeping playback follow, scrubbing, and scrolling responsive.
```

Update `Basic Workflow` with:

```markdown
6. Choose `Manual Track` to create an empty editable cue track for direct authoring.
7. Click cues to select them, shift-click to multi-select, drag selected cues to move them, and drag cue edges to resize duration cues.
8. Use `Undo` and `Redo` to recover from marker and manual-track edits during the current session.
9. Use generated timing tracks as snap guides while editing; hold the snap-bypass modifier for free placement.
```

- [x] **Step 2: Run focused milestone suites**

Run:

```bash
uv run python -m unittest tests.test_editable_marker_inspector tests.test_timeline_model tests.test_waveform_summary tests.test_playback_transport tests.test_app_controller -v
```

Expected: PASS.

- [x] **Step 3: Run full unit suite**

Run:

```bash
uv run python -m unittest discover -s tests -v
```

Expected: PASS.

- [x] **Step 4: Run headless smoke and visual QA**

Run:

```bash
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
QT_QPA_PLATFORM=offscreen uv run python main.py --screenshot /tmp/autolight-interactive-timeline-final.png
uv run python scripts/check_qml_screenshot.py /tmp/autolight-interactive-timeline-final.png
```

Expected: all commands exit 0.

- [x] **Step 5: Check whitespace and final worktree status**

Run:

```bash
git diff --check
git status --short
```

Expected: `git diff --check` exits 0. `git status --short` shows only intended files before plan closure.

- [x] **Step 6: Mark this implementation plan complete**

After all previous steps pass, update every completed checkbox in `docs/superpowers/plans/2026-06-01-autolight-interactive-timeline.md` from `[ ]` to `[x]`.

- [x] **Step 7: Commit documentation and plan closure**

```bash
git add README.md docs/superpowers/plans/2026-06-01-autolight-interactive-timeline.md
git commit -m "Document interactive timeline milestone"
```

Expected: commit succeeds.

## Implementation Notes

- Keep generated tracks read-only. All direct manipulation slots must reject non-editable tracks through Python validation, even if QML disables the controls.
- Manual cue tracks should be source-backed, not standalone, so existing graph validation stays simple and downstream transforms still have a coherent audio context.
- Drag previews can be visual-only in QML. Persist changes only on release through controller slots so undo history receives one command per completed gesture.
- The first snapping target set is visible generated timing markers. Use the controller-visible track list when QML provides one; otherwise all generated timing tracks in the model are acceptable.
- Auto-follow should never update scroll on every playback position signal. Use the viewport policy and keep playhead movement separate from scroll movement.
- Waveform LOD payloads must retain `samples` at the top level for compatibility with existing code until all consumers use `levels`.
- Keep undo history non-persistent and clear it on `new_project`, `open_project`, and `load_demo_project`.

## Self-Review

- Spec coverage: Tasks 3, 5, and 9 cover manual tracks and direct timeline editing. Task 4 covers undo/redo. Tasks 1 and 8 cover Python/QML component extraction. Tasks 2 and 10 cover playback follow, scrubbing, and QML performance. Tasks 6 and 7 cover waveform LOD generation, selection, slicing, and model exposure. Task 11 covers docs and full verification.
- Red-flag scan: no task uses deferred filler text or vague "add tests" language; each test and implementation step names files and expected commands.
- Type consistency: controller slots use snake_case to match existing QML-facing slots; QML property names use existing lower camel case plus explicit new role names.
