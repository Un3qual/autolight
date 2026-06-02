# Autolight Tree-Aware Music Analysis Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add tree-aware timeline projection, parent-artifact transform routing, and librosa-backed beat, energy, and harmonic analysis tracks.

**Architecture:** Keep the existing `ProjectDocument.tracks` graph schema and render it as a flattened expanded tree in `TimelineTrackModel`. Add an app-layer input resolver so audio transforms can run against either source audio or a complete parent audio artifact. Add a replaceable `MusicAnalysisEngine` used by separate built-in transforms that write dense JSON artifacts and practical marker tracks.

**Tech Stack:** Python 3.14, PySide6/QML, librosa, numpy, soundfile, unittest, existing `TransformRegistry`, `LocalJobQueue`, `TimelineTrackModel`, and `AppController`.

---

## File Structure

- Create `autolight/app/transform_inputs.py`: resolve runtime transform input paths from source tracks or parent audio artifacts.
- Create `autolight/app/analysis_lod.py`: return bounded visible slices for energy and harmonic artifact strips.
- Create `autolight/analysis/music.py`: dataclasses and librosa-backed rhythm, energy, and harmony analysis engine.
- Modify `autolight/analysis/builtin.py`: register `audio.drums_stand_in`, `music.beat_grid`, `music.energy_profile`, and `music.harmonic_color`.
- Modify `autolight/app_controller.py`: use transform input resolver, load analysis artifacts into track provenance, persist tree expansion state, and refresh visible analysis slices.
- Modify `autolight/timeline/model.py`: expose tree roles and visible analysis strip roles.
- Modify `UI/components/TimelineView.qml`: route expand/collapse requests and visible-row updates through flattened tree rows.
- Modify `UI/components/TrackRow.qml`: indent nested tracks and show expand/collapse control plus child state.
- Modify `UI/components/TimelineLane.qml`: pass visible analysis samples into a strip component.
- Create `UI/components/AnalysisStrip.qml`: draw energy and harmonic strip data with Canvas.
- Modify `README.md`: document tree-aware analysis workflow.
- Test `tests/test_timeline_model.py`: tree flattening, roles, state summaries, and analysis roles.
- Test `tests/test_app_controller.py`: transform input routing, tree state persistence, QML wiring, and artifact loading.
- Test `tests/test_analysis.py`: music transform registration and marker/artifact output.
- Create `tests/test_music_analysis.py`: focused engine and bounded artifact-slice tests.

## Task 1: Timeline Tree Projection Roles

**Files:**
- Modify: `autolight/timeline/model.py`
- Modify: `tests/test_timeline_model.py`

- [ ] **Step 1: Add failing timeline tree tests**

Add these tests to `TimelineTrackModelTest` in `tests/test_timeline_model.py`:

```python
    def test_model_projects_tracks_as_expanded_tree_rows(self):
        project = new_project("Demo")
        source = Track(id="track_source", type=TrackType.SOURCE, name="Song", result_state=ResultState.COMPLETE)
        drums = Track(
            id="track_drums",
            type=TrackType.GENERATED,
            name="Drums",
            input_track_ids=[source.id],
            result_state=ResultState.COMPLETE,
        )
        onsets = Track(
            id="track_onsets",
            type=TrackType.GENERATED,
            name="Drum Onsets",
            input_track_ids=[drums.id],
            result_state=ResultState.STALE,
        )
        beat_grid = Track(
            id="track_beats",
            type=TrackType.GENERATED,
            name="Beat Grid",
            input_track_ids=[source.id],
            result_state=ResultState.COMPLETE,
        )
        project.tracks.extend([source, drums, onsets, beat_grid])

        model = TimelineTrackModel()
        model.set_project(project)

        self.assertEqual(model.rowCount(), 4)
        ids = [
            model.data(model.index(row, 0), model.role_for_name("trackId"))
            for row in range(model.rowCount())
        ]
        depths = [
            model.data(model.index(row, 0), model.role_for_name("depth"))
            for row in range(model.rowCount())
        ]

        self.assertEqual(ids, ["track_source", "track_drums", "track_onsets", "track_beats"])
        self.assertEqual(depths, [0, 1, 2, 1])
        self.assertEqual(model.data(model.index(0, 0), model.role_for_name("childCount")), 2)
        self.assertTrue(model.data(model.index(0, 0), model.role_for_name("hasChildren")))
        self.assertEqual(model.data(model.index(1, 0), model.role_for_name("parentTrackId")), "track_source")
        self.assertEqual(model.data(model.index(2, 0), model.role_for_name("parentTrackId")), "track_drums")
        self.assertEqual(model.data(model.index(1, 0), model.role_for_name("visibleChildStateSummary")), "stale: 1")

    def test_model_collapses_tree_rows_without_destroying_project_order(self):
        project = new_project("Demo")
        source = Track(id="track_source", type=TrackType.SOURCE, name="Song", result_state=ResultState.COMPLETE)
        child = Track(
            id="track_child",
            type=TrackType.GENERATED,
            name="Child",
            input_track_ids=[source.id],
            result_state=ResultState.COMPLETE,
        )
        sibling = Track(id="track_sibling", type=TrackType.SOURCE, name="Other", result_state=ResultState.COMPLETE)
        project.tracks.extend([source, child, sibling])

        model = TimelineTrackModel()
        model.set_project(project)
        self.assertTrue(model.set_track_expanded(source.id, False))

        ids = [
            model.data(model.index(row, 0), model.role_for_name("trackId"))
            for row in range(model.rowCount())
        ]

        self.assertEqual(ids, ["track_source", "track_sibling"])
        self.assertFalse(model.data(model.index(0, 0), model.role_for_name("expanded")))
        self.assertEqual([track.id for track in project.tracks], ["track_source", "track_child", "track_sibling"])

    def test_model_renders_missing_parent_as_problem_root_row(self):
        project = new_project("Demo")
        orphan = Track(
            id="track_orphan",
            type=TrackType.GENERATED,
            name="Orphan",
            input_track_ids=["missing_parent"],
            result_state=ResultState.COMPLETE,
        )
        project.tracks.append(orphan)

        model = TimelineTrackModel()
        model.set_project(project)

        self.assertEqual(model.rowCount(), 1)
        self.assertEqual(model.data(model.index(0, 0), model.role_for_name("depth")), 0)
        self.assertEqual(
            model.data(model.index(0, 0), model.role_for_name("treeError")),
            "missing parent: missing_parent",
        )
```

- [ ] **Step 2: Run tree model tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_timeline_model.TimelineTrackModelTest -v
```

Expected: fail because roles such as `depth`, `parentTrackId`, `expanded`, `childCount`, `visibleChildStateSummary`, `treeError`, and `set_track_expanded()` do not exist yet.

- [ ] **Step 3: Implement flattened expanded tree projection**

In `autolight/timeline/model.py`, add roles after `waveformLevelBucketCount`:

```python
        Qt.ItemDataRole.UserRole + 18: b"parentTrackId",
        Qt.ItemDataRole.UserRole + 19: b"depth",
        Qt.ItemDataRole.UserRole + 20: b"hasChildren",
        Qt.ItemDataRole.UserRole + 21: b"expanded",
        Qt.ItemDataRole.UserRole + 22: b"childCount",
        Qt.ItemDataRole.UserRole + 23: b"visibleChildStateSummary",
        Qt.ItemDataRole.UserRole + 24: b"treeError",
```

Add instance state in `__init__`:

```python
        self._expanded_track_ids: set[str] = set()
        self._tree_rows: list[Track] = []
        self._tree_depths: dict[str, int] = {}
        self._tree_parents: dict[str, str] = {}
        self._tree_errors: dict[str, str] = {}
        self._children_by_track: dict[str, list[Track]] = {}
```

Add role handlers:

```python
            self.role_for_name("parentTrackId"): lambda track: self._tree_parents.get(track.id, ""),
            self.role_for_name("depth"): lambda track: self._tree_depths.get(track.id, 0),
            self.role_for_name("hasChildren"): lambda track: bool(self._children_by_track.get(track.id)),
            self.role_for_name("expanded"): lambda track: track.id in self._expanded_track_ids,
            self.role_for_name("childCount"): lambda track: len(self._children_by_track.get(track.id, [])),
            self.role_for_name("visibleChildStateSummary"): self._visible_child_state_summary,
            self.role_for_name("treeError"): lambda track: self._tree_errors.get(track.id, ""),
```

Change `set_project()` so it calls `_rebuild_tree_projection()` after `_rebuild_marker_index()`. Change `rowCount()`, `index()`, `_track_for_index()`, and `refresh_track()` to use `self._tree_rows` instead of `self._project.tracks` for row lookup.

Add these methods:

```python
    def set_track_expanded(self, track_id: str, expanded: bool) -> bool:
        if self._project is None:
            return False
        known_track_ids = {track.id for track in self._project.tracks}
        if track_id not in known_track_ids:
            return False
        if expanded:
            if track_id in self._expanded_track_ids:
                return False
            self._expanded_track_ids.add(track_id)
        else:
            if track_id not in self._expanded_track_ids:
                return False
            self._expanded_track_ids.remove(track_id)
        self.beginResetModel()
        self._rebuild_tree_projection()
        self._generation += 1
        self.endResetModel()
        return True

    def expanded_track_ids(self) -> list[str]:
        return sorted(self._expanded_track_ids)

    def set_expanded_track_ids(self, track_ids: list[str]) -> None:
        self._expanded_track_ids = {str(track_id) for track_id in track_ids}
        if self._project is not None:
            self.beginResetModel()
            self._rebuild_tree_projection()
            self._generation += 1
            self.endResetModel()

    def visible_track_ids(self, first_row: int, row_count: int) -> list[str]:
        start = max(0, min(int(first_row), len(self._tree_rows)))
        stop = min(len(self._tree_rows), start + max(0, int(row_count)))
        return [track.id for track in self._tree_rows[start:stop]]

    def _rebuild_tree_projection(self) -> None:
        self._tree_rows = []
        self._tree_depths = {}
        self._tree_parents = {}
        self._tree_errors = {}
        self._children_by_track = {}
        if self._project is None:
            return
        tracks_by_id = {track.id: track for track in self._project.tracks}
        for track in self._project.tracks:
            parent_id = track.input_track_ids[0] if track.input_track_ids else ""
            if parent_id and parent_id in tracks_by_id:
                self._children_by_track.setdefault(parent_id, []).append(track)
                self._tree_parents[track.id] = parent_id
            elif parent_id:
                self._tree_errors[track.id] = f"missing parent: {parent_id}"
        for track in self._project.tracks:
            if self._tree_parents.get(track.id):
                continue
            self._append_tree_row(track, depth=0, active_path=set())

    def _append_tree_row(self, track: Track, depth: int, active_path: set[str]) -> None:
        self._tree_rows.append(track)
        self._tree_depths[track.id] = depth
        if track.id in active_path:
            self._tree_errors[track.id] = "cycle detected"
            return
        if track.id not in self._expanded_track_ids:
            return
        next_path = set(active_path)
        next_path.add(track.id)
        for child in self._children_by_track.get(track.id, []):
            self._append_tree_row(child, depth + 1, next_path)

    def _visible_child_state_summary(self, track: Track) -> str:
        counts: dict[str, int] = {}
        pending = list(self._children_by_track.get(track.id, []))
        seen: set[str] = set()
        while pending:
            child = pending.pop(0)
            if child.id in seen:
                continue
            seen.add(child.id)
            if child.result_state != ResultState.COMPLETE:
                state = child.result_state.value
                counts[state] = counts.get(state, 0) + 1
            pending.extend(self._children_by_track.get(child.id, []))
        return ", ".join(f"{state}: {counts[state]}" for state in sorted(counts))
```

In `set_project()`, initialize expansion for known parent tracks:

```python
        if project is not None:
            parent_ids = {input_id for track in project.tracks for input_id in track.input_track_ids}
            known_ids = {track.id for track in project.tracks}
            self._expanded_track_ids |= parent_ids & known_ids
```

- [ ] **Step 4: Run tree model tests**

Run:

```bash
uv run python -m unittest tests.test_timeline_model.TimelineTrackModelTest -v
```

Expected: all `TimelineTrackModelTest` tests pass.

- [ ] **Step 5: Commit tree projection roles**

```bash
git add autolight/timeline/model.py tests/test_timeline_model.py
git commit -m "Add timeline tree projection roles"
```

Expected: commit succeeds.

## Task 2: Tree Expansion Controller And QML Rows

**Files:**
- Modify: `autolight/app_controller.py`
- Modify: `UI/components/TimelineView.qml`
- Modify: `UI/components/TrackRow.qml`
- Modify: `tests/test_app_controller.py`

- [ ] **Step 1: Add failing controller and QML wiring tests**

Add these tests to `AppControllerTest` in `tests/test_app_controller.py`:

```python
    def test_controller_persists_timeline_tree_expansion_state(self):
        from autolight.project.store import add_generated_track

        controller = self._controller()
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            source_id = controller.import_audio(str(audio_path))
            child = add_generated_track(
                controller._project,
                source_id,
                "Child",
                "markers.fixed_interval",
                {},
                "1",
                "markers.v1",
                "dep",
            )
            controller.trackModel.set_project(controller._project)
            self.assertTrue(controller.set_track_expanded(source_id, False))
            project_path = Path(tmp) / "tree.autolight"
            controller.save_project(str(project_path))

            reopened = self._controller()
            reopened.open_project(str(project_path))
            reopened.trackModel.set_project(reopened._project)

        self.assertEqual(
            reopened._project.ui_state["expanded_track_ids"],
            [],
        )
        self.assertEqual(reopened.trackModel.rowCount(), 1)
        self.assertEqual(child.input_track_ids, [source_id])

    def test_qml_exposes_timeline_tree_controls(self):
        qml = self._qml_text(
            "UI/components/TimelineView.qml",
            "UI/components/TrackRow.qml",
        )

        self.assertIn("required property int depth", qml)
        self.assertIn("required property bool hasChildren", qml)
        self.assertIn("required property bool expanded", qml)
        self.assertIn("required property string visibleChildStateSummary", qml)
        self.assertIn("appController.set_track_expanded(root.trackId, !root.expanded)", qml)
        self.assertIn("leftPadding: 10 + root.depth * 18", qml)
        self.assertIn("text: root.expanded ? \"▾\" : \"▸\"", qml)
```

- [ ] **Step 2: Run controller QML tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_controller_persists_timeline_tree_expansion_state tests.test_app_controller.AppControllerTest.test_qml_exposes_timeline_tree_controls -v
```

Expected: fail because `AppController.set_track_expanded` and QML tree bindings do not exist.

- [ ] **Step 3: Add controller tree state slots**

In `autolight/app_controller.py`, update `set_timeline_visible_track_range()`:

```python
    @Slot(int, int)
    def set_timeline_visible_track_range(self, first_row: int, row_count: int) -> None:
        self._visible_track_ids = self._track_model.visible_track_ids(first_row, row_count)
```

Add a slot:

```python
    @Slot(str, bool, result=bool)
    def set_track_expanded(self, track_id: str, expanded: bool) -> bool:
        changed = self._track_model.set_track_expanded(track_id, expanded)
        if changed:
            self._project.ui_state["expanded_track_ids"] = self._track_model.expanded_track_ids()
            self._mark_non_history_dirty()
        return changed
```

After every `self._track_model.set_project(self._project)` call that happens during project open, demo load, and new project replacement, call:

```python
        expanded_ids = self._project.ui_state.get("expanded_track_ids", [])
        if isinstance(expanded_ids, list):
            self._track_model.set_expanded_track_ids([str(track_id) for track_id in expanded_ids])
```

Use a helper to avoid repetition:

```python
    def _set_track_model_project(self) -> None:
        self._track_model.set_project(self._project)
        expanded_ids = self._project.ui_state.get("expanded_track_ids", [])
        if isinstance(expanded_ids, list):
            self._track_model.set_expanded_track_ids([str(track_id) for track_id in expanded_ids])
```

Replace direct `self._track_model.set_project(self._project)` calls in controller project-replacement paths with `_set_track_model_project()`. Leave focused `refresh_track()` calls unchanged.

- [ ] **Step 4: Add QML tree row controls**

In `UI/components/TimelineView.qml`, pass the new required roles to `TrackRow`:

```qml
        depth: model.depth
        hasChildren: model.hasChildren
        expanded: model.expanded
        visibleChildStateSummary: model.visibleChildStateSummary
        treeError: model.treeError
```

In `UI/components/TrackRow.qml`, add required properties:

```qml
    required property int depth
    required property bool hasChildren
    required property bool expanded
    required property string visibleChildStateSummary
    required property string treeError
```

Inside the label `Column`, replace `anchors.margins: 10` with:

```qml
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            anchors.right: parent.right
            anchors.left: parent.left
            anchors.leftMargin: 10 + root.depth * 18
            anchors.rightMargin: 10
            anchors.topMargin: 10
            anchors.bottomMargin: 10
```

Add an expand/collapse button before the track-name `Text`:

```qml
            Row {
                width: parent.width
                spacing: 6

                Button {
                    width: 24
                    height: 22
                    visible: root.hasChildren
                    text: root.expanded ? "▾" : "▸"
                    onClicked: root.appController.set_track_expanded(root.trackId, !root.expanded)
                }

                Text {
                    text: root.name
                    color: root.textPrimary
                    font.pixelSize: 14
                    elide: Text.ElideRight
                    width: parent.width - (root.hasChildren ? 30 : 0)
                }
            }
```

Update the status/error text to include child summaries and tree errors:

```qml
                text: root.visibleChildStateSummary.length > 0
                    ? root.trackType + " - " + root.resultState + " - " + root.markerCount + " markers - children " + root.visibleChildStateSummary
                    : root.trackType + " - " + root.resultState + " - " + root.markerCount + " markers"
```

```qml
                text: root.treeError.length > 0 ? root.treeError : root.error
                visible: root.error.length > 0 || root.treeError.length > 0
```

- [ ] **Step 5: Run controller and QML tests**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_controller_persists_timeline_tree_expansion_state tests.test_app_controller.AppControllerTest.test_qml_exposes_timeline_tree_controls -v
```

Expected: both tests pass.

- [ ] **Step 6: Run smoke check**

Run:

```bash
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: exits 0.

- [ ] **Step 7: Commit tree UI**

```bash
git add autolight/app_controller.py UI/components/TimelineView.qml UI/components/TrackRow.qml tests/test_app_controller.py
git commit -m "Show timeline tracks as an expandable tree"
```

Expected: commit succeeds.

## Task 3: Parent Artifact Transform Input Routing

**Files:**
- Create: `autolight/app/transform_inputs.py`
- Modify: `autolight/app_controller.py`
- Modify: `autolight/analysis/builtin.py`
- Modify: `tests/test_app_controller.py`
- Modify: `tests/test_analysis.py`

- [ ] **Step 1: Add failing input routing and stand-in audio artifact tests**

Add this test to `AnalysisRegistryTest` in `tests/test_analysis.py`:

```python
    def test_drums_stand_in_writes_audio_artifact(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        transform = registry.get("audio.drums_stand_in", version="1")

        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp) / "source.wav"
            source.write_bytes(b"test audio bytes")
            artifact_dir = Path(tmp) / "artifacts"
            result = transform.run(
                TransformContext(
                    artifact_dir=artifact_dir,
                    cancel_requested=lambda: False,
                    progress=lambda value: None,
                ),
                {"audio_path": str(source)},
            )

        self.assertEqual(set(result.artifacts), {"audio"})
        self.assertEqual(Path(result.artifacts["audio"]).read_bytes(), b"test audio bytes")
```

Add these tests to `AppControllerTest` in `tests/test_app_controller.py`:

```python
    def test_audio_transform_routes_to_parent_audio_artifact(self):
        from autolight.project.store import add_generated_track

        controller = self._controller()
        with tempfile.TemporaryDirectory() as tmp:
            source_audio = Path(tmp) / "song.wav"
            drum_audio = Path(tmp) / "drums.wav"
            write_wav(source_audio)
            write_wav(drum_audio)
            source_id = controller.import_audio(str(source_audio))
            drums = add_generated_track(
                controller._project,
                source_id,
                "Drums",
                "audio.drums_stand_in",
                {},
                "1",
                "artifact.audio.v1",
                "drums-dep",
            )
            drums.result_state = ResultState.COMPLETE
            cache_entry = CacheEntry(
                id="cache_drums",
                dependency_hash="drums-dep",
                artifact_kind="audio",
                path="audio/cache_drums.wav",
                created_at="",
                transform_version="1",
            )
            controller._project.cache_entries.append(cache_entry)
            drums.cache_refs = [cache_entry.id]
            cached_path = controller._job_queue.cache_store.artifact_path(cache_entry)
            cached_path.parent.mkdir(parents=True, exist_ok=True)
            cached_path.write_bytes(drum_audio.read_bytes())

            child_id = controller.add_transform_track(drums.id, "waveform.summary", "1", "{}")
            child = next(track for track in controller._project.tracks if track.id == child_id)
            params = controller._runtime_transform_params_for_track(child)

        self.assertEqual(params["audio_path"], str(cached_path))
        self.assertEqual(child.input_track_ids, [drums.id])

    def test_audio_transform_rejects_stale_parent_artifact(self):
        from autolight.project.store import add_generated_track

        controller = self._controller()
        with tempfile.TemporaryDirectory() as tmp:
            source_audio = Path(tmp) / "song.wav"
            write_wav(source_audio)
            source_id = controller.import_audio(str(source_audio))
            drums = add_generated_track(
                controller._project,
                source_id,
                "Drums",
                "audio.drums_stand_in",
                {},
                "1",
                "artifact.audio.v1",
                "drums-dep",
            )
            drums.result_state = ResultState.STALE

            child_id = controller.add_transform_track(drums.id, "waveform.summary", "1", "{}")

        self.assertEqual(child_id, "")
        self.assertIn("parent track is not complete", controller.lastError)
```

- [ ] **Step 2: Run routing tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_analysis.AnalysisRegistryTest.test_drums_stand_in_writes_audio_artifact tests.test_app_controller.AppControllerTest.test_audio_transform_routes_to_parent_audio_artifact tests.test_app_controller.AppControllerTest.test_audio_transform_rejects_stale_parent_artifact -v
```

Expected: fail because `audio.drums_stand_in` and parent artifact routing do not exist.

- [ ] **Step 3: Add transform input resolver**

Create `autolight/app/transform_inputs.py`:

```python
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from autolight.cache.store import CacheStore
from autolight.project.models import ProjectDocument, ResultState, Track, TrackType
from autolight.project.store import find_track


@dataclass(slots=True)
class TransformInputResolver:
    project: ProjectDocument
    cache_store: CacheStore

    def audio_path_for_track(self, track: Track) -> str:
        if track.type == TrackType.SOURCE:
            return self._source_audio_path(track)
        if track.result_state != ResultState.COMPLETE:
            raise ValueError(f"parent track is not complete: {track.name}")
        return str(self._valid_audio_artifact_path(track))

    def _source_audio_path(self, track: Track) -> str:
        asset_id = track.provenance.get("asset_id")
        for asset in self.project.audio_assets:
            if asset.id == asset_id:
                if asset.import_status != "online":
                    raise ValueError(f"source audio is not online: {track.name}")
                return asset.path
        for parent_id in track.input_track_ids:
            parent = find_track(self.project, parent_id)
            if parent is not None:
                return self.audio_path_for_track(parent)
        raise ValueError(f"source audio path not found for track: {track.name}")

    def _valid_audio_artifact_path(self, track: Track) -> Path:
        entries = {entry.id: entry for entry in self.project.cache_entries}
        for cache_ref in track.cache_refs:
            entry = entries.get(cache_ref)
            if entry is None or entry.artifact_kind != "audio" or entry.validation_status != "valid":
                continue
            path = self.cache_store.artifact_path(entry)
            if path.is_file():
                return path
        raise ValueError(f"parent track has no valid audio artifact: {track.name}")
```

- [ ] **Step 4: Add stand-in audio artifact transform**

In `autolight/analysis/builtin.py`, import `shutil` and register:

```python
    registry.register(
        TransformSpec(
            id="audio.drums_stand_in",
            version="1",
            name="Drums Stem Stand-In",
            input_schema="audio.v1",
            output_schema="artifact.audio.v1",
            estimated_cost="medium",
            run=_drums_stand_in,
        )
    )
```

Add functions:

```python
def _drums_stand_in(context: TransformContext, params: dict) -> TransformResult:
    source = Path(str(params["audio_path"]))
    context.artifact_dir.mkdir(parents=True, exist_ok=True)
    _raise_if_cancelled(context)
    output = Path(context.artifact_dir) / "drums.wav"
    shutil.copyfile(source, output)
    context.progress(1.0)
    return TransformResult(artifacts={"audio": str(output)}, metadata={"stem": "drums"})

```

- [ ] **Step 5: Wire resolver into controller transform defaults and runtime params**

In `autolight/app_controller.py`, import:

```python
from autolight.app.transform_inputs import TransformInputResolver
```

Add helper:

```python
    def _transform_input_resolver(self) -> TransformInputResolver:
        return TransformInputResolver(self._project, self._job_queue.cache_store)
```

Replace source-audio resolution in `_params_with_parent_defaults()`:

```python
        if spec.input_schema == "audio.v1":
            self._transform_input_resolver().audio_path_for_track(parent)
            enriched.pop("audio_path", None)
```

Replace `_runtime_transform_params_for_track()` audio path handling:

```python
        if spec.input_schema == "audio.v1":
            params.pop("audio_path", None)
            parent = find_track(self._project, track.input_track_ids[0]) if track.input_track_ids else track
            if parent is None:
                raise ValueError(f"parent track not found: {track.input_track_ids[0]}")
            params["audio_path"] = self._transform_input_resolver().audio_path_for_track(parent)
```

Keep `_dependency_transform_params_for_track()` removing `audio_path` so cached identity remains path-independent and parent dependency hash still carries the actual parent input identity.

- [ ] **Step 6: Run routing tests**

Run:

```bash
uv run python -m unittest tests.test_analysis.AnalysisRegistryTest.test_drums_stand_in_writes_audio_artifact tests.test_app_controller.AppControllerTest.test_audio_transform_routes_to_parent_audio_artifact tests.test_app_controller.AppControllerTest.test_audio_transform_rejects_stale_parent_artifact -v
```

Expected: all three tests pass.

- [ ] **Step 7: Commit input routing**

```bash
git add autolight/app/transform_inputs.py autolight/app_controller.py autolight/analysis/builtin.py tests/test_app_controller.py tests/test_analysis.py
git commit -m "Route audio transforms through parent artifacts"
```

Expected: commit succeeds.

## Task 4: Music Analysis Engine

**Files:**
- Create: `autolight/analysis/music.py`
- Modify: `autolight/analysis/__init__.py`
- Create: `tests/test_music_analysis.py`

- [ ] **Step 1: Add failing engine tests**

Create `tests/test_music_analysis.py`:

```python
import tempfile
import unittest
import wave
from pathlib import Path

from autolight.analysis.music import MusicAnalysisEngine


def write_impulse_wav(path: Path, *, sample_rate: int = 8000, seconds: float = 2.0) -> None:
    frame_count = int(sample_rate * seconds)
    samples = []
    for index in range(frame_count):
        value = 20000 if index % (sample_rate // 2) == 0 else 0
        samples.append(value.to_bytes(2, "little", signed=True))
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(sample_rate)
        handle.writeframes(b"".join(samples))


class MusicAnalysisEngineTest(unittest.TestCase):
    def test_energy_profile_returns_bounded_normalized_frames(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "impulses.wav"
            write_impulse_wav(audio_path)
            result = MusicAnalysisEngine().analyze_energy(audio_path, {"max_frames": 32})

        self.assertEqual(result.kind, "energy")
        self.assertLessEqual(len(result.frames), 32)
        self.assertTrue(all(0.0 <= frame["intensity"] <= 1.0 for frame in result.frames))
        self.assertTrue(any(marker["category"] == "energy_peak" for marker in result.markers))

    def test_harmony_profile_returns_chroma_color_frames(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "impulses.wav"
            write_impulse_wav(audio_path)
            result = MusicAnalysisEngine().analyze_harmony(audio_path, {"max_frames": 16})

        self.assertEqual(result.kind, "harmonic-color")
        self.assertLessEqual(len(result.frames), 16)
        if result.frames:
            self.assertEqual(len(result.frames[0]["chroma"]), 12)
            self.assertIn("color", result.frames[0])

    def test_beat_grid_returns_artifact_payload_and_marker_dicts(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "impulses.wav"
            write_impulse_wav(audio_path, seconds=4.0)
            result = MusicAnalysisEngine().analyze_rhythm(audio_path, {"max_markers": 64})

        self.assertEqual(result.kind, "beat-grid")
        self.assertIn("version", result.payload)
        self.assertLessEqual(len(result.markers), 64)
        self.assertTrue(all("timestamp" in marker for marker in result.markers))
```

- [ ] **Step 2: Run engine tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_music_analysis -v
```

Expected: fail because `autolight.analysis.music` does not exist.

- [ ] **Step 3: Implement music analysis engine**

Create `autolight/analysis/music.py`:

```python
from __future__ import annotations

import math
import warnings
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import librosa
import numpy as np


DEFAULT_HOP_LENGTH = 512
DEFAULT_MAX_FRAMES = 2048
DEFAULT_MAX_MARKERS = 2048


@dataclass(slots=True)
class MusicAnalysisResult:
    kind: str
    payload: dict[str, Any]
    markers: list[dict[str, Any]] = field(default_factory=list)
    frames: list[dict[str, Any]] = field(default_factory=list)


class MusicAnalysisEngine:
    def analyze_rhythm(self, audio_path: str | Path, settings: dict[str, Any] | None = None) -> MusicAnalysisResult:
        settings = dict(settings or {})
        y, sr = _load_audio(audio_path)
        hop_length = _positive_int(settings.get("hop_length", DEFAULT_HOP_LENGTH), "hop_length")
        max_markers = _positive_int(settings.get("max_markers", DEFAULT_MAX_MARKERS), "max_markers")
        tempo, beat_frames = librosa.beat.beat_track(y=y, sr=sr, hop_length=hop_length, units="frames")
        beat_times = librosa.frames_to_time(beat_frames, sr=sr, hop_length=hop_length)
        onset_env = librosa.onset.onset_strength(y=y, sr=sr, hop_length=hop_length)
        tempo_value = _first_float(tempo)
        markers = []
        for index, timestamp in enumerate(beat_times[:max_markers]):
            beat_strength = _frame_value(onset_env, int(beat_frames[index]) if index < len(beat_frames) else 0)
            category = "downbeat" if index % 4 == 0 else "beat"
            markers.append(
                {
                    "timestamp": round(float(timestamp), 6),
                    "label": "Downbeat" if category == "downbeat" else "Beat",
                    "category": category,
                    "confidence": beat_strength,
                    "metadata": {
                        "beat_index": index,
                        "bar_index": index // 4,
                        "tempo": tempo_value,
                        "meter": 4,
                        "beat_strength": beat_strength,
                        "source": "librosa.beat_track",
                    },
                }
            )
        payload = {
            "version": 1,
            "kind": "beat-grid",
            "duration": _duration_seconds(y, sr),
            "tempo": tempo_value,
            "beat_times": [round(float(value), 6) for value in beat_times[:max_markers]],
            "settings": {"hop_length": hop_length, "max_markers": max_markers},
        }
        return MusicAnalysisResult(kind="beat-grid", payload=payload, markers=markers)

    def analyze_energy(self, audio_path: str | Path, settings: dict[str, Any] | None = None) -> MusicAnalysisResult:
        settings = dict(settings or {})
        y, sr = _load_audio(audio_path)
        hop_length = _positive_int(settings.get("hop_length", DEFAULT_HOP_LENGTH), "hop_length")
        max_frames = _positive_int(settings.get("max_frames", DEFAULT_MAX_FRAMES), "max_frames")
        rms = librosa.feature.rms(y=y, hop_length=hop_length)[0]
        onset_env = librosa.onset.onset_strength(y=y, sr=sr, hop_length=hop_length)
        times = librosa.frames_to_time(np.arange(len(rms)), sr=sr, hop_length=hop_length)
        intensity = _normalize(rms + _resize(onset_env, len(rms)))
        frames = _decimated_frames(times, intensity, max_frames, "intensity")
        markers = _energy_markers(times, intensity)
        payload = {
            "version": 1,
            "kind": "energy",
            "duration": _duration_seconds(y, sr),
            "frames": frames,
            "settings": {"hop_length": hop_length, "max_frames": max_frames},
        }
        return MusicAnalysisResult(kind="energy", payload=payload, markers=markers, frames=frames)

    def analyze_harmony(self, audio_path: str | Path, settings: dict[str, Any] | None = None) -> MusicAnalysisResult:
        settings = dict(settings or {})
        y, sr = _load_audio(audio_path)
        hop_length = _positive_int(settings.get("hop_length", DEFAULT_HOP_LENGTH), "hop_length")
        max_frames = _positive_int(settings.get("max_frames", DEFAULT_MAX_FRAMES), "max_frames")
        chroma = librosa.feature.chroma_cqt(y=y, sr=sr, hop_length=hop_length)
        times = librosa.frames_to_time(np.arange(chroma.shape[1]), sr=sr, hop_length=hop_length)
        frames = _chroma_frames(times, chroma, max_frames)
        markers = _harmonic_change_markers(frames)
        payload = {
            "version": 1,
            "kind": "harmonic-color",
            "duration": _duration_seconds(y, sr),
            "frames": frames,
            "settings": {"hop_length": hop_length, "max_frames": max_frames},
        }
        return MusicAnalysisResult(kind="harmonic-color", payload=payload, markers=markers, frames=frames)
```

Append helper functions in the same file:

```python
def _load_audio(audio_path: str | Path):
    with warnings.catch_warnings():
        warnings.filterwarnings("ignore", message="n_fft=.*", category=UserWarning)
        return librosa.load(str(audio_path), sr=None, mono=True)


def _positive_int(value: Any, name: str) -> int:
    try:
        result = int(value)
    except (TypeError, ValueError, OverflowError) as exc:
        raise ValueError(f"{name} must be a positive integer") from exc
    if result <= 0:
        raise ValueError(f"{name} must be a positive integer")
    return result


def _first_float(value: Any) -> float:
    values = np.asarray(value).reshape(-1)
    if values.size == 0:
        return 0.0
    result = float(values[0])
    return result if math.isfinite(result) else 0.0


def _duration_seconds(y: np.ndarray, sr: int) -> float:
    return 0.0 if sr <= 0 else float(len(y) / sr)


def _frame_value(values: np.ndarray, index: int) -> float:
    if len(values) == 0:
        return 0.0
    return float(max(0.0, min(1.0, _normalize(values)[max(0, min(index, len(values) - 1))])))


def _resize(values: np.ndarray, size: int) -> np.ndarray:
    if len(values) == size:
        return values
    if size <= 0:
        return np.asarray([])
    if len(values) == 0:
        return np.zeros(size)
    return np.interp(np.linspace(0, len(values) - 1, size), np.arange(len(values)), values)


def _normalize(values: np.ndarray) -> np.ndarray:
    values = np.asarray(values, dtype=float)
    if values.size == 0:
        return values
    min_value = float(np.nanmin(values))
    max_value = float(np.nanmax(values))
    if not math.isfinite(min_value) or not math.isfinite(max_value) or max_value <= min_value:
        return np.zeros_like(values, dtype=float)
    return np.clip((values - min_value) / (max_value - min_value), 0.0, 1.0)


def _decimated_frames(times: np.ndarray, values: np.ndarray, max_frames: int, value_key: str) -> list[dict[str, float]]:
    if len(values) == 0:
        return []
    stride = max(1, math.ceil(len(values) / max_frames))
    return [
        {"time": round(float(times[index]), 6), value_key: round(float(values[index]), 6)}
        for index in range(0, len(values), stride)
    ][:max_frames]


def _energy_markers(times: np.ndarray, intensity: np.ndarray) -> list[dict[str, Any]]:
    if len(intensity) == 0:
        return []
    threshold = max(0.65, float(np.mean(intensity) + np.std(intensity)))
    markers = []
    for index in range(1, len(intensity) - 1):
        value = float(intensity[index])
        if value >= threshold and value >= intensity[index - 1] and value >= intensity[index + 1]:
            markers.append(
                {
                    "timestamp": round(float(times[index]), 6),
                    "label": "Energy Peak",
                    "category": "energy_peak",
                    "confidence": round(value, 6),
                    "metadata": {"intensity": round(value, 6), "source": "rms_onset_intensity"},
                }
            )
    return markers[:256]


def _chroma_frames(times: np.ndarray, chroma: np.ndarray, max_frames: int) -> list[dict[str, Any]]:
    if chroma.size == 0:
        return []
    stride = max(1, math.ceil(chroma.shape[1] / max_frames))
    frames = []
    for frame_index in range(0, chroma.shape[1], stride):
        vector = np.asarray(chroma[:, frame_index], dtype=float)
        normalized = _normalize(vector)
        dominant = int(np.argmax(normalized)) if normalized.size else 0
        frames.append(
            {
                "time": round(float(times[frame_index]), 6),
                "chroma": [round(float(value), 6) for value in normalized[:12]],
                "color": _color_for_pitch_class(dominant),
                "dominant_pitch_class": dominant,
            }
        )
    return frames[:max_frames]


def _color_for_pitch_class(pitch_class: int) -> str:
    hue = int((pitch_class % 12) * 30)
    return f"hsl({hue}, 72%, 58%)"


def _harmonic_change_markers(frames: list[dict[str, Any]]) -> list[dict[str, Any]]:
    markers = []
    previous = None
    for frame in frames:
        current = frame.get("dominant_pitch_class")
        if previous is not None and current != previous:
            markers.append(
                {
                    "timestamp": float(frame["time"]),
                    "label": "Harmonic Change",
                    "category": "harmonic_change",
                    "confidence": 0.75,
                    "metadata": {"previous_pitch_class": previous, "pitch_class": current},
                }
            )
        previous = current
    return markers[:256]
```

Update `autolight/analysis/__init__.py`:

```python
from autolight.analysis.music import MusicAnalysisEngine, MusicAnalysisResult
```

and add both names to `__all__`.

- [ ] **Step 4: Run engine tests**

Run:

```bash
uv run python -m unittest tests.test_music_analysis -v
```

Expected: all tests pass.

- [ ] **Step 5: Commit music analysis engine**

```bash
git add autolight/analysis/music.py autolight/analysis/__init__.py tests/test_music_analysis.py
git commit -m "Add librosa music analysis engine"
```

Expected: commit succeeds.

## Task 5: Music Analysis Transforms And Artifacts

**Files:**
- Modify: `autolight/analysis/builtin.py`
- Modify: `tests/test_analysis.py`

- [ ] **Step 1: Add failing transform registration and artifact tests**

Add this helper to `tests/test_analysis.py`:

```python
def write_impulse_wav(path: Path) -> None:
    import wave

    sample_rate = 8000
    samples = []
    for index in range(sample_rate * 2):
        value = 18000 if index % (sample_rate // 2) == 0 else 0
        samples.append(value.to_bytes(2, "little", signed=True))
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(sample_rate)
        handle.writeframes(b"".join(samples))
```

Add these tests to `AnalysisRegistryTest`:

```python
    def test_builtin_registry_contains_music_analysis_transforms(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        self.assertIn("music.beat_grid", registry.ids())
        self.assertIn("music.energy_profile", registry.ids())
        self.assertIn("music.harmonic_color", registry.ids())

    def test_music_analysis_transforms_write_expected_artifacts(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        for transform_id, artifact_kind in [
            ("music.beat_grid", "beat-grid"),
            ("music.energy_profile", "energy"),
            ("music.harmonic_color", "harmonic-color"),
        ]:
            with self.subTest(transform_id=transform_id):
                with tempfile.TemporaryDirectory() as tmp:
                    audio_path = Path(tmp) / "song.wav"
                    write_impulse_wav(audio_path)
                    transform = registry.get(transform_id, version="1")
                    result = transform.run(
                        TransformContext(
                            artifact_dir=Path(tmp) / "artifacts",
                            cancel_requested=lambda: False,
                            progress=lambda value: None,
                        ),
                        {"audio_path": str(audio_path), "max_frames": 32, "max_markers": 64},
                    )
                    artifact = Path(result.artifacts[artifact_kind])
                    payload = json.loads(artifact.read_text(encoding="utf-8"))

                self.assertEqual(payload["version"], 1)
                self.assertEqual(payload["kind"], artifact_kind)
                self.assertIn("settings", payload)

    def test_music_analysis_transform_cancels_before_loading_audio(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        transform = registry.get("music.energy_profile", version="1")

        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(TransformCancelled):
                transform.run(
                    TransformContext(
                        artifact_dir=Path(tmp) / "artifacts",
                        cancel_requested=lambda: True,
                        progress=lambda value: None,
                    ),
                    {"audio_path": str(Path(tmp) / "missing.wav")},
                )
```

- [ ] **Step 2: Run transform tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_analysis.AnalysisRegistryTest.test_builtin_registry_contains_music_analysis_transforms tests.test_analysis.AnalysisRegistryTest.test_music_analysis_transforms_write_expected_artifacts tests.test_analysis.AnalysisRegistryTest.test_music_analysis_transform_cancels_before_loading_audio -v
```

Expected: fail because music transforms are not registered yet.

- [ ] **Step 3: Implement music transform runners**

In `autolight/analysis/builtin.py`, import:

```python
from autolight.analysis.music import MusicAnalysisEngine
```

Register the music transforms with individual runners inside `register_builtin_transforms()`:

```python
    registry.register(
        TransformSpec(
            id="music.beat_grid",
            version="1",
            name="Beat Grid",
            input_schema="audio.v1",
            output_schema="artifact.beat-grid.v1",
            estimated_cost="medium",
            run=_music_beat_grid,
        )
    )
    registry.register(
        TransformSpec(
            id="music.energy_profile",
            version="1",
            name="Energy Profile",
            input_schema="audio.v1",
            output_schema="artifact.energy.v1",
            estimated_cost="medium",
            run=_music_energy_profile,
        )
    )
    registry.register(
        TransformSpec(
            id="music.harmonic_color",
            version="1",
            name="Harmonic Color",
            input_schema="audio.v1",
            output_schema="artifact.harmonic-color.v1",
            estimated_cost="medium",
            run=_music_harmonic_color,
        )
    )
```

Add helpers:

```python
def _music_beat_grid(context: TransformContext, params: dict) -> TransformResult:
    return _run_music_analysis(context, params, "beat-grid", MusicAnalysisEngine().analyze_rhythm)


def _music_energy_profile(context: TransformContext, params: dict) -> TransformResult:
    return _run_music_analysis(context, params, "energy", MusicAnalysisEngine().analyze_energy)


def _music_harmonic_color(context: TransformContext, params: dict) -> TransformResult:
    return _run_music_analysis(context, params, "harmonic-color", MusicAnalysisEngine().analyze_harmony)


def _run_music_analysis(context: TransformContext, params: dict, artifact_kind: str, analyzer) -> TransformResult:
    _raise_if_cancelled(context)
    context.progress(0.05)
    audio_path = Path(str(params["audio_path"]))
    settings = {key: value for key, value in params.items() if key != "audio_path"}
    result = analyzer(audio_path, settings)
    _raise_if_cancelled(context)
    context.artifact_dir.mkdir(parents=True, exist_ok=True)
    artifact_path = Path(context.artifact_dir) / f"{artifact_kind}.json"
    artifact_path.write_text(json.dumps(result.payload, sort_keys=True), encoding="utf-8")
    context.progress(1.0)
    return TransformResult(
        markers=result.markers,
        artifacts={artifact_kind: str(artifact_path)},
        metadata={"kind": artifact_kind},
    )
```

- [ ] **Step 4: Run transform tests**

Run:

```bash
uv run python -m unittest tests.test_analysis tests.test_music_analysis -v
```

Expected: all analysis and music analysis tests pass.

- [ ] **Step 5: Commit music transforms**

```bash
git add autolight/analysis/builtin.py tests/test_analysis.py
git commit -m "Register music analysis transforms"
```

Expected: commit succeeds.

## Task 6: Analysis Artifact Loading And Timeline Roles

**Files:**
- Create: `autolight/app/analysis_lod.py`
- Modify: `autolight/app_controller.py`
- Modify: `autolight/timeline/model.py`
- Modify: `tests/test_music_analysis.py`
- Modify: `tests/test_timeline_model.py`

- [ ] **Step 1: Add failing artifact slice and model role tests**

Add to `tests/test_music_analysis.py`:

```python
from autolight.app.analysis_lod import AnalysisLodStore


class AnalysisLodStoreTest(unittest.TestCase):
    def test_visible_frames_returns_bounded_time_window(self):
        payload = {
            "version": 1,
            "kind": "energy",
            "duration": 10.0,
            "frames": [{"time": float(index), "intensity": index / 10.0} for index in range(10)],
        }
        visible = AnalysisLodStore().visible_frames(
            payload,
            scroll_seconds=2.0,
            visible_seconds=3.0,
            max_frames=4,
        )

        self.assertEqual([frame["time"] for frame in visible["frames"]], [2.0, 3.0, 4.0, 5.0])
        self.assertEqual(visible["kind"], "energy")
```

Add to `TimelineTrackModelTest`:

```python
    def test_model_exposes_visible_analysis_samples_for_complete_valid_artifacts(self):
        project = new_project("Demo")
        energy = Track(
            id="track_energy",
            type=TrackType.GENERATED,
            name="Energy",
            result_state=ResultState.COMPLETE,
            cache_refs=["cache_energy"],
            provenance={
                "visible_energy": {
                    "kind": "energy",
                    "frames": [{"time": 0.0, "intensity": 0.5}],
                }
            },
        )
        harmonic = Track(
            id="track_harmony",
            type=TrackType.GENERATED,
            name="Harmony",
            result_state=ResultState.COMPLETE,
            cache_refs=["cache_harmony"],
            provenance={
                "visible_harmonic_color": {
                    "kind": "harmonic-color",
                    "frames": [{"time": 0.0, "color": "hsl(0, 72%, 58%)"}],
                }
            },
        )
        project.tracks.extend([energy, harmonic])
        project.cache_entries.extend(
            [
                CacheEntry("cache_energy", "dep", "energy", "energy.json", "", "1"),
                CacheEntry("cache_harmony", "dep", "harmonic-color", "harmony.json", "", "1"),
            ]
        )
        model = TimelineTrackModel()
        model.set_project(project)

        self.assertEqual(
            model.data(model.index(0, 0), model.role_for_name("visibleEnergySamples"))[0]["intensity"],
            0.5,
        )
        self.assertEqual(
            model.data(model.index(1, 0), model.role_for_name("visibleHarmonicColorSamples"))[0]["color"],
            "hsl(0, 72%, 58%)",
        )
```

- [ ] **Step 2: Run artifact role tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_music_analysis.AnalysisLodStoreTest tests.test_timeline_model.TimelineTrackModelTest.test_model_exposes_visible_analysis_samples_for_complete_valid_artifacts -v
```

Expected: fail because `AnalysisLodStore` and analysis roles do not exist.

- [ ] **Step 3: Implement analysis LOD store**

Create `autolight/app/analysis_lod.py`:

```python
from __future__ import annotations

import math
from typing import Any


class AnalysisLodStore:
    def visible_frames(
        self,
        payload: dict[str, Any],
        *,
        scroll_seconds: float,
        visible_seconds: float,
        max_frames: int = 256,
    ) -> dict[str, Any]:
        frames = payload.get("frames", [])
        if not isinstance(frames, list):
            frames = []
        start = max(0.0, _finite_float(scroll_seconds))
        stop = start + max(0.0, _finite_float(visible_seconds))
        visible = [
            dict(frame)
            for frame in frames
            if isinstance(frame, dict)
            and start <= _finite_float(frame.get("time", 0.0)) <= stop
        ]
        if len(visible) > max_frames:
            stride = max(1, math.ceil(len(visible) / max_frames))
            visible = visible[::stride][:max_frames]
        return {
            "kind": str(payload.get("kind", "")),
            "duration": max(0.0, _finite_float(payload.get("duration", 0.0))),
            "frames": visible,
        }


def _finite_float(value) -> float:
    try:
        result = float(value)
    except (TypeError, ValueError, OverflowError):
        return 0.0
    return result if math.isfinite(result) else 0.0
```

- [ ] **Step 4: Add timeline model analysis roles**

In `autolight/timeline/model.py`, add roles:

```python
        Qt.ItemDataRole.UserRole + 25: b"visibleEnergySamples",
        Qt.ItemDataRole.UserRole + 26: b"visibleHarmonicColorSamples",
```

Add handlers:

```python
            self.role_for_name("visibleEnergySamples"): lambda track: self._visible_analysis_frames(track, "energy", "visible_energy"),
            self.role_for_name("visibleHarmonicColorSamples"): lambda track: self._visible_analysis_frames(track, "harmonic-color", "visible_harmonic_color"),
```

Add methods:

```python
    def _visible_analysis_frames(self, track: Track, artifact_kind: str, provenance_key: str) -> list:
        if not self._has_complete_valid_artifact(track, artifact_kind):
            return []
        visible = track.provenance.get(provenance_key, {})
        if not isinstance(visible, dict):
            return []
        frames = visible.get("frames", [])
        if not isinstance(frames, list):
            return []
        return [dict(frame) for frame in frames if isinstance(frame, dict)]

    def _has_complete_valid_artifact(self, track: Track, artifact_kind: str) -> bool:
        if self._project is None or track.result_state != ResultState.COMPLETE:
            return False
        entries = {entry.id: entry for entry in self._project.cache_entries}
        return any(
            (entry := entries.get(cache_ref)) is not None
            and entry.artifact_kind == artifact_kind
            and entry.validation_status == "valid"
            for cache_ref in track.cache_refs
        )
```

- [ ] **Step 5: Load visible analysis artifacts in controller**

In `autolight/app_controller.py`, import:

```python
from autolight.app.analysis_lod import AnalysisLodStore
```

Construct:

```python
        self._analysis_lod = AnalysisLodStore()
```

Add methods:

```python
    def _load_all_analysis_artifacts(self) -> None:
        for track in self._project.tracks:
            self._load_analysis_artifacts(track.id)

    def _load_analysis_artifacts(self, track_id: str) -> None:
        track = find_track(self._project, track_id)
        if track is None:
            return
        entries = {entry.id: entry for entry in self._project.cache_entries}
        for cache_ref in track.cache_refs:
            entry = entries.get(cache_ref)
            if entry is None or entry.validation_status != "valid":
                continue
            if entry.artifact_kind not in {"energy", "harmonic-color"}:
                continue
            path = self._job_queue.cache_store.artifact_path(entry)
            try:
                payload = json.loads(path.read_text(encoding="utf-8"))
            except (OSError, json.JSONDecodeError):
                continue
            visible = self._analysis_lod.visible_frames(
                payload,
                scroll_seconds=self._timeline_scroll_seconds,
                visible_seconds=self.timelineVisibleSeconds,
            )
            if entry.artifact_kind == "energy":
                track.provenance["visible_energy"] = visible
            elif entry.artifact_kind == "harmonic-color":
                track.provenance["visible_harmonic_color"] = visible
```

Call `_load_all_analysis_artifacts()` after project load/demo load and after cache refresh. Call `_load_analysis_artifacts(track_id)` in `_handle_track_changed()` before emitting `trackChangedRequested`.

- [ ] **Step 6: Run artifact role tests**

Run:

```bash
uv run python -m unittest tests.test_music_analysis.AnalysisLodStoreTest tests.test_timeline_model.TimelineTrackModelTest.test_model_exposes_visible_analysis_samples_for_complete_valid_artifacts -v
```

Expected: both tests pass.

- [ ] **Step 7: Commit analysis artifact roles**

```bash
git add autolight/app/analysis_lod.py autolight/app_controller.py autolight/timeline/model.py tests/test_music_analysis.py tests/test_timeline_model.py
git commit -m "Expose visible music analysis artifact slices"
```

Expected: commit succeeds.

## Task 7: QML Analysis Strips

**Files:**
- Create: `UI/components/AnalysisStrip.qml`
- Modify: `UI/components/TimelineLane.qml`
- Modify: `UI/components/TrackRow.qml`
- Modify: `UI/components/TimelineView.qml`
- Modify: `tests/test_app_controller.py`

- [ ] **Step 1: Add failing QML analysis strip test**

Add to `AppControllerTest`:

```python
    def test_qml_renders_energy_and_harmonic_analysis_strips(self):
        qml = self._qml_text(
            "UI/components/TimelineView.qml",
            "UI/components/TrackRow.qml",
            "UI/components/TimelineLane.qml",
            "UI/components/AnalysisStrip.qml",
        )

        self.assertIn("required property var visibleEnergySamples", qml)
        self.assertIn("required property var visibleHarmonicColorSamples", qml)
        self.assertIn("AnalysisStrip", qml)
        self.assertIn('stripKind: "energy"', qml)
        self.assertIn('stripKind: "harmonic-color"', qml)
        self.assertIn("sample.intensity", qml)
        self.assertIn("sample.color", qml)
        self.assertIn("Canvas", qml)
```

- [ ] **Step 2: Run QML test and verify failure**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_qml_renders_energy_and_harmonic_analysis_strips -v
```

Expected: fail because `AnalysisStrip.qml` and QML bindings do not exist.

- [ ] **Step 3: Add analysis strip component**

Create `UI/components/AnalysisStrip.qml`:

```qml
import QtQuick

Canvas {
    id: root
    required property var samples
    required property string stripKind
    property real durationSeconds: 0
    property color energyColor: "#facc15"
    width: parent ? parent.width : 0
    height: 16
    visible: root.samples.length > 0

    onSamplesChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()

    onPaint: {
        var ctx = getContext("2d")
        ctx.clearRect(0, 0, width, height)
        if (root.samples.length === 0 || width <= 0 || height <= 0) {
            return
        }
        for (var index = 0; index < root.samples.length; index += 1) {
            var sample = root.samples[index]
            var x = index / Math.max(1, root.samples.length - 1) * width
            if (root.stripKind === "energy") {
                var intensity = Math.max(0, Math.min(1, Number(sample.intensity || 0)))
                ctx.strokeStyle = root.energyColor
                ctx.beginPath()
                ctx.moveTo(x, height)
                ctx.lineTo(x, height - intensity * height)
                ctx.stroke()
            } else {
                ctx.fillStyle = sample.color || "#93c5fd"
                ctx.fillRect(x, 0, Math.max(1, width / root.samples.length), height)
            }
        }
    }
}
```

- [ ] **Step 4: Wire strips through timeline QML**

In `TrackRow.qml`, add required properties:

```qml
    required property var visibleEnergySamples
    required property var visibleHarmonicColorSamples
```

Pass them into `TimelineLane`:

```qml
        visibleEnergySamples: root.visibleEnergySamples
        visibleHarmonicColorSamples: root.visibleHarmonicColorSamples
```

In `TimelineView.qml`, pass model roles:

```qml
        visibleEnergySamples: model.visibleEnergySamples
        visibleHarmonicColorSamples: model.visibleHarmonicColorSamples
```

In `TimelineLane.qml`, add required or default properties:

```qml
    property var visibleEnergySamples: []
    property var visibleHarmonicColorSamples: []
```

Add strips above marker blocks:

```qml
    AnalysisStrip {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 18
        samples: root.visibleEnergySamples
        stripKind: "energy"
    }

    AnalysisStrip {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 2
        samples: root.visibleHarmonicColorSamples
        stripKind: "harmonic-color"
    }
```

- [ ] **Step 5: Run QML and smoke tests**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_qml_renders_energy_and_harmonic_analysis_strips -v
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: test passes and smoke exits 0.

- [ ] **Step 6: Commit analysis strips**

```bash
git add UI/components/AnalysisStrip.qml UI/components/TimelineLane.qml UI/components/TrackRow.qml UI/components/TimelineView.qml tests/test_app_controller.py
git commit -m "Render music analysis strips in timeline lanes"
```

Expected: commit succeeds.

## Task 8: Demo Workflow, Documentation, And Final Verification

**Files:**
- Modify: `autolight/app_controller.py`
- Modify: `README.md`
- Modify: `docs/superpowers/plans/2026-06-02-autolight-tree-aware-music-analysis.md`

- [ ] **Step 1: Add demo project child tracks**

In `AppController.load_demo_project()`, after the source track is created, add tree-aware demo tracks:

```python
        drums = add_generated_track(
            self._project,
            parent_track_id=source.id,
            name="Drums Stem",
            transform_id="audio.drums_stand_in",
            transform_params={},
            transform_version="1",
            output_schema="artifact.audio.v1",
            dependency_hash="demo-drums",
        )
        drums.result_state = ResultState.PENDING
        add_generated_track(
            self._project,
            parent_track_id=drums.id,
            name="Drum Energy",
            transform_id="music.energy_profile",
            transform_params={},
            transform_version="1",
            output_schema="artifact.energy.v1",
            dependency_hash="demo-drum-energy",
        )
        self._project.ui_state["expanded_track_ids"] = [source.id, drums.id]
```

If demo creation already creates related tracks, preserve them and add these as additional nested examples.

- [ ] **Step 2: Update README current scope and workflow**

Add bullets under `## Current Scope` in `README.md`:

```markdown
- Display generated and editable tracks as a nested transform tree.
- Route child audio analysis transforms through parent audio artifacts when available.
- Generate beat-grid, energy-profile, and harmonic-color analysis tracks with dense cache artifacts.
- Render energy and harmonic/color analysis strips in the timeline.
```

Add workflow steps after generated-track creation:

```markdown
7. Add `Drums Stem Stand-In` under a source track when you want a nested audio-artifact branch.
8. Add beat-grid, energy-profile, or harmonic-color transforms under a source or compatible audio-artifact track.
9. Expand or collapse parent tracks to inspect nested analysis outputs.
```

Renumber subsequent workflow steps.

- [ ] **Step 3: Run focused verification**

Run:

```bash
uv run python -m unittest tests.test_analysis tests.test_music_analysis tests.test_timeline_model -v
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_qml_exposes_timeline_tree_controls tests.test_app_controller.AppControllerTest.test_qml_renders_energy_and_harmonic_analysis_strips -v
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
git diff --check
```

Expected: all tests pass, smoke exits 0, and `git diff --check` exits 0.

- [ ] **Step 4: Run full verification**

Run:

```bash
uv run python -m unittest discover -s tests -v
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: full unit suite passes and smoke exits 0.

- [ ] **Step 5: Mark this implementation plan complete**

After all previous steps pass, update every completed checkbox in `docs/superpowers/plans/2026-06-02-autolight-tree-aware-music-analysis.md` from `[ ]` to `[x]`.

- [ ] **Step 6: Commit documentation and plan closure**

```bash
git add autolight/app_controller.py README.md docs/superpowers/plans/2026-06-02-autolight-tree-aware-music-analysis.md
git commit -m "Document tree-aware music analysis workflow"
```

Expected: commit succeeds.

## Implementation Notes

- Keep export, song-structure labels, and real source separation out of this milestone.
- Use `audio.drums_stand_in` only to prove artifact routing and nested UI behavior.
- Do not change the `.autolight` schema version; tree state belongs in optional `ui_state` keys.
- Preserve existing generated-track stale propagation. Parent reruns must stale nested child tracks through existing dependency traversal.
- Keep dense artifact payloads bounded. Prefer decimation over emitting thousands of QML delegates.
- If harmonic-change markers are noisy on fixtures, keep markers conservative and rely on the dense color strip for user-visible harmonic value.

## Self-Review

- Spec coverage: Tasks 1 and 2 cover timeline tree projection and UI. Task 3 covers parent artifact routing and stand-in audio artifacts. Tasks 4 and 5 cover the replaceable librosa engine and product transforms. Tasks 6 and 7 cover dense artifact strips. Task 8 covers demo/docs/final verification.
- Red-flag scan: no task uses TBD, TODO, "add tests for above", or unnamed implementation steps.
- Type consistency: role names are `parentTrackId`, `depth`, `hasChildren`, `expanded`, `childCount`, `visibleChildStateSummary`, `treeError`, `visibleEnergySamples`, and `visibleHarmonicColorSamples`; QML and tests use the same names.
