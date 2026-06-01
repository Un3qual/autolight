from __future__ import annotations

import copy
import json
import math
import tempfile
import time
from pathlib import Path

from PySide6.QtCore import Property, QObject, Qt, QUrl, Signal, Slot

from autolight.app import (
    EditHistory,
    MarkerEditingService,
    ProjectSession,
    TimelineViewport,
    WaveformLodStore,
)
from autolight.app.edit_history import MarkerSnapshotCommand, ProjectSnapshotCommand, TrackSnapshotCommand
from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformRegistry
from autolight.cache.keys import track_dependency_hash
from autolight.demo_audio import write_silent_wav
from autolight.jobs.queue import LocalJobQueue
from autolight.playback import PlaybackTransport
from autolight.project.models import AudioAsset, Marker, ResultState, TrackType
from autolight.project.store import (
    DEFAULT_MARKER_COLOR,
    MARKER_COLOR_PALETTE,
    ProjectStore,
    add_editable_marker,
    add_generated_track,
    bulk_update_editable_markers,
    create_editable_track_from_markers,
    create_manual_editable_track,
    delete_editable_marker,
    find_track,
    import_audio_asset,
    marker_snapshot,
    marker_color_key,
    marker_display_color,
    move_editable_markers,
    new_project,
    normalize_marker_category,
    normalize_marker_color,
    refresh_audio_asset_status,
    refresh_audio_track_status,
    resize_editable_marker,
    track_dependency_inputs,
    update_editable_marker,
)
from autolight.timeline.model import TimelineTrackModel
from autolight.timeline.transform_model import TransformSpecModel


TIMELINE_UI_STATE_KEY = "timeline"
TIMELINE_DEFAULT_PIXELS_PER_SECOND = 96.0


class AppController(QObject):
    projectNameChanged = Signal()
    projectPathChanged = Signal()
    lastErrorChanged = Signal()
    selectedTrackIdChanged = Signal()
    selectedTrackMarkersChanged = Signal()
    selectedMarkerIdsChanged = Signal()
    selectedTrackHasRunningJobChanged = Signal()
    selectedTrackCanRerunChanged = Signal()
    selectedTrackCanPlayChanged = Signal()
    timelineDurationSecondsChanged = Signal()
    timelinePixelsPerSecondChanged = Signal()
    timelineScrollSecondsChanged = Signal()
    timelineVisibleSecondsChanged = Signal()
    isDirtyChanged = Signal()
    canUndoChanged = Signal()
    canRedoChanged = Signal()
    _track_changed_on_main_thread = Signal(str)

    def __init__(self):
        super().__init__()
        self._project = new_project("Untitled")
        self._session = ProjectSession(self._project)
        self._marker_editing = MarkerEditingService()
        self._edit_history = EditHistory()
        self._viewport = TimelineViewport()
        self._waveform_lod = WaveformLodStore()
        self._project_path = ""
        self._last_error = ""
        self._selected_track_id = ""
        self._selected_marker_ids: list[str] = []
        self._is_dirty = False
        self._non_history_dirty = False
        self._demo_temp_dir: tempfile.TemporaryDirectory | None = None
        self._runtime_temp_dir = tempfile.TemporaryDirectory(prefix="autolight-runtime-")
        self._playback = PlaybackTransport(parent=self)
        self._playback.durationSecondsChanged.connect(self._notify_timeline_duration_changed)
        self._playback.positionSecondsChanged.connect(self._keep_playback_position_visible)
        self._timeline_pixels_per_second = TIMELINE_DEFAULT_PIXELS_PER_SECOND
        self._timeline_scroll_seconds = 0.0
        self._timeline_visible_seconds = 8.0
        self._track_model = TimelineTrackModel(parent=self)
        self._track_model.set_project(self._project)
        self._registry = TransformRegistry()
        register_builtin_transforms(self._registry)
        self._transform_model = TransformSpecModel(self._registry, parent=self)
        self._job_queue = LocalJobQueue(
            self._registry,
            artifact_root=Path(self._runtime_temp_dir.name) / "artifacts",
            on_track_changed=self._queue_track_changed,
        )
        self._track_changed_on_main_thread.connect(
            self._handle_track_changed,
            Qt.ConnectionType.QueuedConnection,
        )

    @Property(QObject, constant=True)
    def trackModel(self):
        return self._track_model

    @Property(QObject, constant=True)
    def transformModel(self):
        return self._transform_model

    @Property(QObject, constant=True)
    def playback(self):
        return self._playback

    @Property(bool, notify=canUndoChanged)
    def canUndo(self) -> bool:
        return self._edit_history.can_undo

    @Property(bool, notify=canRedoChanged)
    def canRedo(self) -> bool:
        return self._edit_history.can_redo

    @Property(list, constant=True)
    def markerColorOptions(self) -> list[dict[str, str]]:
        return [
            {"key": key, "label": key.title(), "color": color}
            for key, color in MARKER_COLOR_PALETTE.items()
        ]

    @Property(bool, notify=selectedTrackCanPlayChanged)
    def selectedTrackCanPlay(self) -> bool:
        asset = self._source_audio_asset_for_track_id(self._selected_track_id)
        return asset is not None and asset.import_status == "online"

    @Property(float, notify=timelineDurationSecondsChanged)
    def timelineDurationSeconds(self) -> float:
        return self._timeline_duration_seconds()

    @Property(float, notify=timelinePixelsPerSecondChanged)
    def timelinePixelsPerSecond(self) -> float:
        return self._timeline_pixels_per_second

    @Property(float, notify=timelineScrollSecondsChanged)
    def timelineScrollSeconds(self) -> float:
        return self._timeline_scroll_seconds

    @Property(float, notify=timelineVisibleSecondsChanged)
    def timelineVisibleSeconds(self) -> float:
        return self._timeline_visible_seconds

    @Property(str, notify=projectNameChanged)
    def projectName(self) -> str:
        return self._project.name

    @Property(str, notify=projectPathChanged)
    def projectPath(self) -> str:
        return self._session.project_path

    @Property(str, notify=lastErrorChanged)
    def lastError(self) -> str:
        return self._last_error

    @Property(str, notify=selectedTrackIdChanged)
    def selectedTrackId(self) -> str:
        return self._selected_track_id

    @Property(list, notify=selectedTrackMarkersChanged)
    def selectedTrackMarkers(self) -> list[dict]:
        return self._marker_summary_for_track(self._selected_track_id)

    @Property(list, notify=selectedMarkerIdsChanged)
    def selectedMarkerIds(self) -> list[str]:
        return list(self._selected_marker_ids)

    @Property(bool, notify=selectedTrackCanRerunChanged)
    def selectedTrackCanRerun(self) -> bool:
        track = find_track(self._project, self._selected_track_id)
        return track is not None and bool(track.transform_id) and self._track_inputs_are_complete(track)

    @Property(bool, notify=selectedTrackIdChanged)
    def selectedTrackIsEditable(self) -> bool:
        track = find_track(self._project, self._selected_track_id)
        return track is not None and track.type == TrackType.EDITABLE

    @Property(bool, notify=selectedTrackHasRunningJobChanged)
    def selectedTrackHasRunningJob(self) -> bool:
        return bool(self._active_job_id_for_track(self._selected_track_id))

    @Property(bool, notify=isDirtyChanged)
    def isDirty(self) -> bool:
        return self._session.dirty

    @Slot()
    def new_project(self) -> None:
        if not self._can_replace_project():
            return
        self._set_project(new_project("Untitled"))
        self._set_project_path("")
        self._set_selected_track_id("")
        self._set_last_error("")
        self._mark_clean()

    @Slot(str, result=bool)
    def open_project(self, path: str) -> bool:
        try:
            self._raise_if_running_jobs("replace project")
            project_path = self._path_from_qml(path)
            project = ProjectStore.load(project_path)
            changed_running_state = self._mark_running_state_stale(project)
            changed_audio_asset_ids = refresh_audio_asset_status(project, search_dirs=[project_path.parent])
            changed_audio_track_ids = refresh_audio_track_status(project)
            self._set_project(project, restore_ui_state=True)
            self._set_project_path(str(project_path))
            self._set_last_error("")
            invalid_cache_refs = self.refresh_cache_status()
            self.selectedTrackCanRerunChanged.emit()
            self._non_history_dirty = bool(
                invalid_cache_refs
                or changed_running_state
                or changed_audio_asset_ids
                or changed_audio_track_ids
            )
            self._sync_dirty_from_history()
            return True
        except Exception as exc:
            self._set_last_error(str(exc))
            return False

    @Slot(str, result=bool)
    def save_project(self, path: str = "") -> bool:
        try:
            if not path and not self._project_path:
                raise ValueError("project path is required")
            project_path = self._path_from_qml(path) if path else Path(self._project_path)
            if project_path.suffix != ".autolight":
                project_path = project_path.with_suffix(".autolight")
            self._raise_if_running_jobs("save project")
            self._capture_timeline_ui_state()
            ProjectStore.save(self._project, project_path)
            self._set_project_path(str(project_path))
            self._set_last_error("")
            self._mark_clean()
            return True
        except Exception as exc:
            self._set_last_error(str(exc))
            return False

    @Slot(str, result=str)
    def import_audio(self, path: str) -> str:
        try:
            audio_path = self._path_from_qml(path)
            track = import_audio_asset(self._project, audio_path)
            self._track_model.set_project(self._project)
            self._set_selected_track_id(track.id)
            self._notify_timeline_duration_changed()
            self._set_last_error("")
            self._mark_non_history_dirty()
            return track.id
        except FileNotFoundError as exc:
            self._set_last_error(f"No such file: {exc}")
            return ""
        except Exception as exc:
            self._set_last_error(str(exc))
            return ""

    @Slot()
    def load_demo_project(self) -> None:
        if not self._can_replace_project():
            return
        if self._demo_temp_dir is not None:
            self._playback.unload()
            self._demo_temp_dir.cleanup()
        self._demo_temp_dir = tempfile.TemporaryDirectory(prefix="autolight-demo-")
        demo_audio_name = Path(self._demo_temp_dir.name).name
        demo_audio_path = Path(self._demo_temp_dir.name) / f"{demo_audio_name}.wav"
        write_silent_wav(demo_audio_path)

        self._set_project(new_project("Autolight Demo"))
        self._set_project_path("")
        source = import_audio_asset(self._project, demo_audio_path)
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
                Marker(id="marker_demo_1", track_id=beats.id, timestamp=0.0, label="Beat", category="timing"),
                Marker(id="marker_demo_2", track_id=beats.id, timestamp=0.5, label="Beat", category="timing"),
                Marker(id="marker_demo_3", track_id=beats.id, timestamp=1.0, label="Beat", category="timing"),
            ]
        )
        create_editable_track_from_markers(self._project, beats.id, "Editable Cues", ["marker_demo_1", "marker_demo_2"])
        waveform = add_generated_track(
            self._project,
            parent_track_id=source.id,
            name="Waveform Summary",
            transform_id="waveform.summary",
            transform_params={"buckets": 80},
            transform_version="1",
            output_schema="artifact.waveform.v1",
            dependency_hash="demo-waveform",
        )
        waveform.result_state = ResultState.COMPLETE
        self._attach_demo_waveform(waveform)
        self._track_model.set_project(self._project)
        self._refresh_visible_waveforms()
        self._set_selected_track_id(source.id)
        self._notify_timeline_duration_changed()
        self._set_last_error("")
        self._mark_clean()

    @Slot(str)
    def select_track(self, track_id: str) -> None:
        if find_track(self._project, track_id) is None:
            self._set_last_error(f"track not found: {track_id}")
            return
        self._set_selected_track_id(track_id)
        self._set_last_error("")

    @Slot(str, float, float, result=str)
    def add_fixed_interval_track(self, parent_track_id: str, duration: float, interval: float) -> str:
        try:
            parent = find_track(self._project, parent_track_id)
            if parent is None:
                raise ValueError(f"parent track not found: {parent_track_id}")
            transform_id = "markers.fixed_interval"
            transform_version = "1"
            params = {"duration": float(duration), "interval": float(interval)}
            dependency_hash = track_dependency_hash(
                track_dependency_inputs(self._project, parent),
                transform_id,
                transform_version,
                params,
            )
            track = add_generated_track(
                self._project,
                parent_track_id=parent.id,
                name="Fixed Interval Markers",
                transform_id=transform_id,
                transform_params=params,
                transform_version=transform_version,
                output_schema="markers.v1",
                dependency_hash=dependency_hash,
            )
            self._track_model.set_project(self._project)
            self._set_selected_track_id(track.id)
            self._notify_timeline_duration_changed()
            self._set_last_error("")
            self._mark_non_history_dirty()
            return track.id
        except Exception as exc:
            self._set_last_error(str(exc))
            return ""

    @Slot(str, result=str)
    def add_vocals_stem_track(self, parent_track_id: str) -> str:
        try:
            parent = find_track(self._project, parent_track_id)
            if parent is None:
                raise ValueError(f"parent track not found: {parent_track_id}")
            transform_id = "stems.vocals_stand_in"
            transform_version = "1"
            params = {"label": "vocals"}
            self._require_source_audio_path_for_track(parent)
            dependency_hash = track_dependency_hash(
                track_dependency_inputs(self._project, parent),
                transform_id,
                transform_version,
                params,
            )
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
            self._notify_timeline_duration_changed()
            self._set_last_error("")
            self._mark_non_history_dirty()
            return track.id
        except Exception as exc:
            self._set_last_error(str(exc))
            return ""

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
            dependency_hash = track_dependency_hash(
                track_dependency_inputs(self._project, parent),
                spec.id,
                spec.version,
                params,
            )
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
            self._notify_timeline_duration_changed()
            self._set_last_error("")
            self._mark_non_history_dirty()
            return track.id
        except Exception as exc:
            self._set_last_error(str(exc))
            return ""

    @Slot(str, result=str)
    def create_editable_track_from_track(self, source_track_id: str) -> str:
        try:
            if find_track(self._project, source_track_id) is None:
                raise ValueError(f"track not found: {source_track_id}")
            marker_ids = [
                marker.id for marker in self._project.markers if marker.track_id == source_track_id
            ]
            if not marker_ids:
                raise ValueError("source track has no markers")
            track = create_editable_track_from_markers(
                self._project,
                source_track_id,
                "Editable Cues",
                marker_ids,
            )
            self._track_model.set_project(self._project)
            self._set_selected_track_id(track.id)
            self._notify_timeline_duration_changed()
            self._set_last_error("")
            self._mark_non_history_dirty()
            return track.id
        except Exception as exc:
            self._set_last_error(str(exc))
            return ""

    @Slot(result=str)
    @Slot(str, result=str)
    def add_manual_cue_track(self, name: str = "Manual Cues") -> str:
        try:
            track = create_manual_editable_track(
                self._project,
                self._selected_track_id,
                name or "Manual Cues",
            )
            track_index = self._project.tracks.index(track)
            self.trackModel.set_project(self._project)
            self._set_selected_track_id(track.id)
            self._notify_timeline_duration_changed()
            self._push_track_creation_command(track, track_index)
            self._sync_dirty_from_history()
            self._set_last_error("")
            return track.id
        except Exception as exc:
            self._set_last_error(str(exc))
            return ""

    @Slot(float, str, result=str)
    @Slot(float, str, str, str, result=str)
    def add_marker_to_selected_track(
        self,
        timestamp: float,
        label: str,
        category: str = "cue",
        color: str = DEFAULT_MARKER_COLOR,
    ) -> str:
        return self._add_marker_to_selected_track(
            timestamp,
            label,
            category=category,
            color=color,
            duration=None,
        )

    @Slot(float, float, str, str, str, result=str)
    def add_marker_to_selected_track_with_duration(
        self,
        timestamp: float,
        duration: float,
        label: str,
        category: str,
        color: str,
    ) -> str:
        return self._add_marker_to_selected_track(
            timestamp,
            label,
            category=category,
            color=color,
            duration=duration,
        )

    def _add_marker_to_selected_track(
        self,
        timestamp: float,
        label: str,
        *,
        category: str,
        color: str,
        duration: float | None,
    ) -> str:
        try:
            before_dependents = self._dependent_track_state_snapshots(self._selected_track_id)
            marker = add_editable_marker(
                self._project,
                self._selected_track_id,
                timestamp,
                label,
                duration=duration,
                category=normalize_marker_category(category),
                color=normalize_marker_color(color),
            )
            after = [marker_snapshot(marker)]
            self._push_marker_snapshot_command(
                self._selected_track_id,
                before=[],
                after=after,
                before_dependents=before_dependents,
                after_dependents=self._dependent_track_state_snapshots(self._selected_track_id),
            )
            self._track_model.set_project(self._project)
            self._set_selected_marker_ids([marker.id], emit_marker_summary=False)
            self.selectedTrackMarkersChanged.emit()
            self._notify_timeline_duration_changed()
            self._set_last_error("")
            self._sync_dirty_from_history()
            return marker.id
        except Exception as exc:
            self._set_last_error(str(exc))
            return ""

    @Slot(str, result=bool)
    def delete_marker_from_selected_track(self, marker_id: str) -> bool:
        return self._delete_markers_from_selected_track([marker_id]) > 0

    @Slot(result=int)
    def delete_selected_markers(self) -> int:
        if not self._selected_marker_ids:
            self._set_last_error("select at least one marker to delete")
            return 0
        return self._delete_markers_from_selected_track(list(self._selected_marker_ids))

    def _delete_markers_from_selected_track(self, marker_ids: list[str]) -> int:
        try:
            before_dependents = self._dependent_track_state_snapshots(self._selected_track_id)
            requested_ids = set(marker_ids)
            before = [
                marker_snapshot(marker)
                for marker in self._project.markers
                if marker.track_id == self._selected_track_id and marker.id in requested_ids
            ]
            deleted_ids = []
            for marker_id in marker_ids:
                if delete_editable_marker(self._project, self._selected_track_id, marker_id):
                    deleted_ids.append(marker_id)
            if deleted_ids:
                self._push_marker_snapshot_command(
                    self._selected_track_id,
                    before=before,
                    after=[],
                    before_dependents=before_dependents,
                    after_dependents=self._dependent_track_state_snapshots(self._selected_track_id),
                )
            self._track_model.set_project(self._project)
            ids_to_clear = set(deleted_ids) if deleted_ids else requested_ids
            if ids_to_clear:
                self._set_selected_marker_ids(
                    [
                        selected_id
                        for selected_id in self._selected_marker_ids
                        if selected_id not in ids_to_clear
                    ],
                    emit_marker_summary=False,
                )
            self.selectedTrackMarkersChanged.emit()
            self._set_last_error("")
            if deleted_ids:
                self._notify_timeline_duration_changed()
                self._sync_dirty_from_history()
            return len(deleted_ids)
        except Exception as exc:
            self._set_last_error(str(exc))
            return 0

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
        return self._update_selected_marker(
            timestamp,
            label,
            category,
            color,
            duration=None,
        )

    @Slot(float, float, str, str, str, result=bool)
    def update_selected_marker_with_duration(
        self,
        timestamp: float,
        duration: float,
        label: str,
        category: str,
        color: str,
    ) -> bool:
        return self._update_selected_marker(
            timestamp,
            label,
            category,
            color,
            duration=duration,
        )

    def _update_selected_marker(
        self,
        timestamp: float,
        label: str,
        category: str,
        color: str,
        *,
        duration: float | None,
    ) -> bool:
        try:
            if len(self._selected_marker_ids) != 1:
                raise ValueError("select one marker to update")
            marker = self._editable_marker_for_selected_marker_id(self._selected_marker_ids[0])
            before = [marker_snapshot(marker)]
            before_dependents = self._dependent_track_state_snapshots(self._selected_track_id)
            update_editable_marker(
                self._project,
                self._selected_track_id,
                self._selected_marker_ids[0],
                timestamp=timestamp,
                duration=duration,
                label=label,
                category=category,
                color=color,
            )
            after = [marker_snapshot(marker)]
            changed = before != after
            if changed:
                self._push_marker_snapshot_command(
                    self._selected_track_id,
                    before=before,
                    after=after,
                    before_dependents=before_dependents,
                    after_dependents=self._dependent_track_state_snapshots(self._selected_track_id),
                )
                self._track_model.set_project(self._project)
                self.selectedTrackMarkersChanged.emit()
                self._notify_timeline_duration_changed()
            self._set_last_error("")
            if changed:
                self._sync_dirty_from_history()
            return True
        except Exception as exc:
            self._set_last_error(str(exc))
            return False

    @Slot(str, str, str, result=int)
    def bulk_update_selected_markers(self, label: str, category: str, color: str) -> int:
        try:
            before = self._marker_snapshots_for_track(self._selected_track_id, self._selected_marker_ids)
            before_dependents = self._dependent_track_state_snapshots(self._selected_track_id)
            updated = bulk_update_editable_markers(
                self._project,
                self._selected_track_id,
                self._selected_marker_ids,
                label=label,
                category=category,
                color=color,
            )
            if not updated:
                self._set_last_error("")
                return 0
            after = self._marker_snapshots_for_track(
                self._selected_track_id,
                [item["id"] for item in before],
            )
            before_changed, after_changed = self._changed_marker_snapshots(before, after)
            self._push_marker_snapshot_command(
                self._selected_track_id,
                before=before_changed,
                after=after_changed,
                before_dependents=before_dependents,
                after_dependents=self._dependent_track_state_snapshots(self._selected_track_id),
            )
            self._track_model.set_project(self._project)
            self.selectedTrackMarkersChanged.emit()
            self._set_last_error("")
            self._sync_dirty_from_history()
            return updated
        except Exception as exc:
            self._set_last_error(str(exc))
            return 0

    @Slot(float, result=bool)
    @Slot(float, bool, result=bool)
    def move_selected_markers(self, delta_seconds: float, bypass_snap: bool = False) -> bool:
        try:
            if not self._selected_marker_ids:
                raise ValueError("select at least one marker to move")
            delta = float(delta_seconds)
            if not math.isfinite(delta):
                raise ValueError("marker move delta must be finite")
            before = [
                marker_snapshot(self._editable_marker_for_selected_marker_id(marker_id))
                for marker_id in self._selected_marker_ids
            ]
            before_dependents = self._dependent_track_state_snapshots(self._selected_track_id)
            if not bypass_snap and len(self._selected_marker_ids) == 1:
                marker = self._editable_marker_for_selected_marker_id(self._selected_marker_ids[0])
                snapped = self.snap_timeline_time(marker.timestamp + delta, False)
                delta = snapped - marker.timestamp
            moved = move_editable_markers(
                self._project,
                self._selected_track_id,
                self._selected_marker_ids,
                delta,
            )
            after = [marker_snapshot(marker) for marker in moved]
            self._push_marker_snapshot_command(
                self._selected_track_id,
                before=before,
                after=after,
                before_dependents=before_dependents,
                after_dependents=self._dependent_track_state_snapshots(self._selected_track_id),
            )
            if before != after:
                self.trackModel.refresh_track(self._selected_track_id)
                self.selectedTrackMarkersChanged.emit()
                self._notify_timeline_duration_changed()
                self._sync_dirty_from_history()
            self._set_last_error("")
            return True
        except Exception as exc:
            self._set_last_error(str(exc))
            return False

    @Slot(str, float, result=bool)
    def resize_marker(self, marker_id: str, duration: float) -> bool:
        try:
            marker = self._editable_marker_for_selected_marker_id(marker_id)
            before = [marker_snapshot(marker)]
            before_dependents = self._dependent_track_state_snapshots(self._selected_track_id)
            updated = resize_editable_marker(
                self._project,
                self._selected_track_id,
                marker_id,
                duration,
            )
            after = [marker_snapshot(updated)]
            self._push_marker_snapshot_command(
                self._selected_track_id,
                before=before,
                after=after,
                before_dependents=before_dependents,
                after_dependents=self._dependent_track_state_snapshots(self._selected_track_id),
            )
            if before != after:
                self.trackModel.refresh_track(self._selected_track_id)
                self.selectedTrackMarkersChanged.emit()
                self._notify_timeline_duration_changed()
                self._sync_dirty_from_history()
            self._set_last_error("")
            return True
        except Exception as exc:
            self._set_last_error(str(exc))
            return False

    @Slot(float, result=float)
    @Slot(float, bool, result=float)
    def snap_timeline_time(self, seconds: float, bypass_snap: bool = False) -> float:
        return self._marker_editing.snap_time(
            self._project,
            requested_seconds=seconds,
            pixels_per_second=self._timeline_pixels_per_second,
            visible_track_ids=[track.id for track in self._project.tracks],
            bypass=bypass_snap,
        )

    @Slot(str, result=str)
    def run_track(self, track_id: str) -> str:
        try:
            return self._submit_track(track_id)
        except Exception as exc:
            self._set_last_error(str(exc))
            return ""

    @Slot()
    def cancel_selected_job(self) -> None:
        job_id = self._active_job_id_for_track(self._selected_track_id)
        if not job_id:
            self._set_last_error("selected track has no running job")
            return
        self.cancel_job(job_id)
        self._set_last_error("")

    @Slot(str, result=str)
    def rerun_track(self, track_id: str) -> str:
        try:
            return self._submit_track(track_id)
        except Exception as exc:
            self._set_last_error(str(exc))
            return ""

    @Slot(str)
    def cancel_job(self, job_id: str) -> None:
        self._job_queue.cancel(job_id)

    @Slot(result=list)
    def refresh_cache_status(self) -> list[str]:
        try:
            invalid_refs = self._job_queue.refresh_cache_validity(self._project)
            self._load_all_waveform_samples()
            self._refresh_visible_waveforms()
            self._track_model.set_project(self._project)
            self.selectedTrackCanRerunChanged.emit()
            if invalid_refs:
                self._mark_non_history_dirty()
                self._set_last_error(f"invalid cache artifacts: {len(invalid_refs)}")
            else:
                self._set_last_error("")
            return invalid_refs
        except Exception as exc:
            self._set_last_error(str(exc))
            return []

    @Slot(result=bool)
    def undo(self) -> bool:
        try:
            if not self._edit_history.undo(self._project):
                return False
            self._reconcile_selection_with_project()
            self.trackModel.set_project(self._project)
            self._refresh_visible_waveforms()
            self.selectedTrackMarkersChanged.emit()
            self._notify_timeline_duration_changed()
            self._notify_history_changed()
            self._sync_dirty_from_history()
            return True
        except Exception as exc:
            self._set_last_error(str(exc))
            return False

    @Slot(result=bool)
    def redo(self) -> bool:
        try:
            if not self._edit_history.redo(self._project):
                return False
            self._reconcile_selection_with_project()
            self.trackModel.set_project(self._project)
            self._refresh_visible_waveforms()
            self.selectedTrackMarkersChanged.emit()
            self._notify_timeline_duration_changed()
            self._notify_history_changed()
            self._sync_dirty_from_history()
            return True
        except Exception as exc:
            self._set_last_error(str(exc))
            return False

    @Slot(result=bool)
    def play_selected_track(self) -> bool:
        asset = self._source_audio_asset_for_track_id(self._selected_track_id)
        if asset is None:
            self._set_last_error("selected track has no source audio")
            return False
        if asset.import_status != "online":
            self._set_last_error(f"source audio is {asset.import_status}")
            return False
        loaded_source_path = self._playback.property("sourcePath")
        if loaded_source_path != asset.path and not self._playback.load_source(
            asset.path,
            asset.duration,
        ):
            self._set_last_error(self._playback.property("lastError"))
            return False
        self._playback.play()
        self._set_last_error("")
        return True

    @Slot()
    def pause_playback(self) -> None:
        self._playback.pause()

    @Slot()
    def stop_playback(self) -> None:
        self._playback.stop()

    @Slot(float)
    def seek_playback(self, seconds: float) -> None:
        self._playback.seek_seconds(seconds)
        self.set_timeline_scroll_seconds(self._scroll_for_visible_time(self._playback.positionSeconds))

    @Slot(float)
    def nudge_playback(self, delta_seconds: float) -> None:
        self.seek_playback(self._playback.positionSeconds + float(delta_seconds))

    @Slot(float)
    def set_timeline_zoom(self, pixels_per_second: float) -> None:
        value = float(pixels_per_second)
        if not math.isfinite(value):
            return
        visible_seconds = self._visible_timeline_seconds()
        playback_position = float(self._playback.positionSeconds)
        visible_start = self._timeline_scroll_seconds
        visible_end = visible_start + visible_seconds
        playback_position_visible = (
            bool(self._playback.sourcePath)
            and visible_start <= playback_position <= visible_end
        )
        anchor = (
            playback_position
            if playback_position_visible
            else self._timeline_scroll_seconds + visible_seconds / 2
        )
        clamped, next_scroll = self._viewport.zoom_around_anchor(
            current_zoom=self._timeline_pixels_per_second,
            requested_zoom=value,
            current_scroll=self._timeline_scroll_seconds,
            visible_seconds=visible_seconds,
            duration_seconds=self._timeline_duration_seconds(),
            anchor_seconds=anchor,
        )
        if self._timeline_pixels_per_second == clamped:
            return
        next_visible_seconds = max(
            0.01,
            visible_seconds * self._timeline_pixels_per_second / clamped,
        )
        self._timeline_pixels_per_second = clamped
        self.timelinePixelsPerSecondChanged.emit()
        if self._timeline_visible_seconds != next_visible_seconds:
            self._timeline_visible_seconds = next_visible_seconds
            self.timelineVisibleSecondsChanged.emit()
        self._set_timeline_scroll_seconds(next_scroll, refresh_visible_waveforms=False)
        self._refresh_visible_waveforms()

    @Slot(float)
    def set_timeline_scroll_seconds(self, seconds: float) -> None:
        self._set_timeline_scroll_seconds(seconds)

    def _set_timeline_scroll_seconds(
        self,
        seconds: float,
        *,
        refresh_visible_waveforms: bool = True,
    ) -> None:
        value = float(seconds)
        if not math.isfinite(value):
            return
        clamped = self._viewport.clamp_scroll(
            value,
            visible_seconds=self._visible_timeline_seconds(),
            duration_seconds=self._timeline_duration_seconds(),
        )
        if self._timeline_scroll_seconds == clamped:
            return
        self._timeline_scroll_seconds = clamped
        self.timelineScrollSecondsChanged.emit()
        if refresh_visible_waveforms:
            self._refresh_visible_waveforms()

    @Slot(float)
    def set_timeline_visible_seconds(self, seconds: float) -> None:
        value = float(seconds)
        if not math.isfinite(value):
            return
        clamped = max(value, 0.01)
        if self._timeline_visible_seconds == clamped:
            return
        self._timeline_visible_seconds = clamped
        self.timelineVisibleSecondsChanged.emit()
        self._set_timeline_scroll_seconds(
            self._timeline_scroll_seconds,
            refresh_visible_waveforms=False,
        )
        self._refresh_visible_waveforms()

    @Slot()
    def cleanup(self) -> None:
        self._job_queue.shutdown()
        self._playback.unload()
        if self._demo_temp_dir is not None:
            self._demo_temp_dir.cleanup()
            self._demo_temp_dir = None
        self._runtime_temp_dir.cleanup()

    def _set_project(self, project, *, restore_ui_state: bool = False) -> None:
        self._playback.unload()
        self._project = project
        self._session.project = project
        self._load_all_waveform_samples()
        self._track_model.set_project(self._project)
        self.set_timeline_zoom(TIMELINE_DEFAULT_PIXELS_PER_SECOND)
        self._timeline_scroll_seconds = 0.0
        self._refresh_visible_waveforms()
        self._set_selected_track_id("")
        self.projectNameChanged.emit()
        self.selectedTrackCanRerunChanged.emit()
        self.selectedTrackCanPlayChanged.emit()
        self._notify_timeline_duration_changed()
        self.timelineScrollSecondsChanged.emit()
        if restore_ui_state:
            self._restore_timeline_ui_state()
            self._refresh_visible_waveforms()
        self._edit_history.clear()
        self._non_history_dirty = False
        self._notify_history_changed()

    def _set_project_path(self, path: str) -> None:
        if self._project_path == path:
            return
        self._project_path = path
        self._session.set_project_path(path)
        self.projectPathChanged.emit()

    def _set_last_error(self, message: str) -> None:
        if self._last_error == message:
            return
        self._last_error = message
        self.lastErrorChanged.emit()

    def _set_selected_track_id(self, track_id: str) -> None:
        if self._selected_track_id == track_id:
            return
        self._selected_track_id = track_id
        self._set_selected_marker_ids([], emit_marker_summary=False)
        self.selectedTrackIdChanged.emit()
        self.selectedTrackMarkersChanged.emit()
        self.selectedTrackHasRunningJobChanged.emit()
        self.selectedTrackCanRerunChanged.emit()
        self.selectedTrackCanPlayChanged.emit()

    def _set_selected_marker_ids(self, marker_ids: list[str], *, emit_marker_summary: bool = True) -> None:
        if self._selected_marker_ids == marker_ids:
            return
        self._selected_marker_ids = list(marker_ids)
        self._track_model.set_selected_marker_ids(self._selected_marker_ids)
        self.selectedMarkerIdsChanged.emit()
        if emit_marker_summary:
            self.selectedTrackMarkersChanged.emit()

    def _set_dirty(self, dirty: bool) -> None:
        if self._is_dirty == dirty:
            return
        self._is_dirty = dirty
        self._session.set_dirty(dirty)
        self.isDirtyChanged.emit()

    def _mark_clean(self) -> None:
        self._non_history_dirty = False
        self._edit_history.mark_clean()
        self._set_dirty(False)

    def _mark_non_history_dirty(self) -> None:
        self._non_history_dirty = True
        self._set_dirty(True)

    def _sync_dirty_from_history(self) -> None:
        self._set_dirty(self._non_history_dirty or not self._edit_history.is_clean())

    def _capture_timeline_ui_state(self) -> None:
        if not isinstance(self._project.ui_state, dict):
            self._project.ui_state = {}
        self._project.ui_state[TIMELINE_UI_STATE_KEY] = {
            "selected_track_id": self._selected_track_id,
            "pixels_per_second": self._timeline_pixels_per_second,
            "scroll_seconds": self._timeline_scroll_seconds,
        }

    def _restore_timeline_ui_state(self) -> None:
        ui_state = self._project.ui_state
        if not isinstance(ui_state, dict):
            return
        state = ui_state.get(TIMELINE_UI_STATE_KEY, {})
        if not isinstance(state, dict):
            return
        pixels_per_second = self._optional_float(state.get("pixels_per_second"))
        self._restore_timeline_zoom(
            pixels_per_second
            if pixels_per_second is not None
            else TIMELINE_DEFAULT_PIXELS_PER_SECOND
        )
        selected_track_id = state.get("selected_track_id", "")
        if (
            isinstance(selected_track_id, str)
            and find_track(self._project, selected_track_id) is not None
        ):
            self._set_selected_track_id(selected_track_id)
        else:
            self._set_selected_track_id("")
        scroll_seconds = self._optional_float(state.get("scroll_seconds"))
        if scroll_seconds is not None:
            self.set_timeline_scroll_seconds(scroll_seconds)

    def _restore_timeline_zoom(self, pixels_per_second: float) -> None:
        visible_seconds = self._visible_timeline_seconds()
        clamped = self._viewport.clamp_zoom(pixels_per_second)
        if self._timeline_pixels_per_second == clamped:
            return
        next_visible_seconds = max(
            0.01,
            visible_seconds * self._timeline_pixels_per_second / clamped,
        )
        self._timeline_pixels_per_second = clamped
        self.timelinePixelsPerSecondChanged.emit()
        if self._timeline_visible_seconds != next_visible_seconds:
            self._timeline_visible_seconds = next_visible_seconds
            self.timelineVisibleSecondsChanged.emit()
        self._set_timeline_scroll_seconds(
            self._timeline_scroll_seconds,
            refresh_visible_waveforms=False,
        )
        self._refresh_visible_waveforms()

    @staticmethod
    def _optional_float(value) -> float | None:
        if value is None or isinstance(value, bool):
            return None
        try:
            result = float(value)
        except (OverflowError, TypeError, ValueError):
            return None
        if not math.isfinite(result):
            return None
        return result

    def _queue_track_changed(self, track_id: str) -> None:
        self._track_changed_on_main_thread.emit(track_id)

    @Slot(str)
    def _handle_track_changed(self, track_id: str) -> None:
        self._load_waveform_samples(track_id)
        self._refresh_visible_waveforms(track_ids={track_id})
        self._track_model.trackChangedRequested.emit(track_id)
        self.selectedTrackCanRerunChanged.emit()
        self._notify_timeline_duration_changed()
        if track_id == self._selected_track_id:
            self.selectedTrackMarkersChanged.emit()
            self.selectedTrackHasRunningJobChanged.emit()

    def _params_with_parent_defaults(self, parent, spec, params: dict) -> dict:
        enriched = dict(params)
        if spec.input_schema == "audio.v1":
            self._require_source_audio_path_for_track(parent)
            enriched.pop("audio_path", None)
        return enriched

    def _require_source_audio_path_for_track(self, track) -> str:
        audio_path = self._source_audio_path_for_track(track)
        if not audio_path:
            raise ValueError("audio transform requires a source audio track")
        return audio_path

    def _source_audio_path_for_track(self, track) -> str:
        asset = self._source_audio_asset_for_track(track)
        return asset.path if asset is not None else ""

    def _source_audio_asset_for_track_id(self, track_id: str) -> AudioAsset | None:
        track = find_track(self._project, track_id)
        if track is None:
            return None
        return self._source_audio_asset_for_track(track)

    def _source_audio_asset_for_track(self, track) -> AudioAsset | None:
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
                return asset
            next_track_ids = list(current.input_track_ids)
            source_track_id = current.provenance.get("source_track_id", "")
            if source_track_id:
                next_track_ids.append(source_track_id)
            for next_track_id in next_track_ids:
                candidate = find_track(self._project, next_track_id)
                if candidate is not None:
                    pending.append(candidate)
        return None

    def _timeline_duration_seconds(self) -> float:
        audio_duration = max((asset.duration for asset in self._project.audio_assets), default=0.0)
        marker_duration = max(
            (
                marker.timestamp + (marker.duration or 0.0)
                for marker in self._project.markers
            ),
            default=0.0,
        )
        return max(audio_duration, marker_duration, self._playback.durationSeconds)

    def _notify_timeline_duration_changed(self) -> None:
        self.timelineDurationSecondsChanged.emit()
        self.set_timeline_scroll_seconds(self._timeline_scroll_seconds)

    def _notify_history_changed(self) -> None:
        self.canUndoChanged.emit()
        self.canRedoChanged.emit()

    def _push_marker_snapshot_command(
        self,
        track_id: str,
        before: list[dict],
        after: list[dict],
        *,
        before_dependents: list[dict] | None = None,
        after_dependents: list[dict] | None = None,
    ) -> None:
        if before == after:
            return
        self._edit_history.push(
            MarkerSnapshotCommand(
                track_id=track_id,
                before=before,
                after=after,
                before_dependents=before_dependents or [],
                after_dependents=after_dependents or [],
            )
        )
        self._notify_history_changed()
        self._sync_dirty_from_history()

    def _push_project_snapshot_command(self, before_project) -> None:
        self._edit_history.push(
            ProjectSnapshotCommand(
                before=before_project,
                after=copy.deepcopy(self._project),
            )
        )
        self._notify_history_changed()
        self._sync_dirty_from_history()

    def _push_track_creation_command(self, track, index: int) -> None:
        self._edit_history.push(
            TrackSnapshotCommand(
                track_id=track.id,
                before=None,
                after=track,
                index=index,
                after_markers=[
                    marker for marker in self._project.markers if marker.track_id == track.id
                ],
                after_job_runs=[
                    job_run for job_run in self._project.job_runs if job_run.track_id == track.id
                ],
            )
        )
        self._notify_history_changed()
        self._sync_dirty_from_history()

    def _marker_snapshots_for_track(self, track_id: str, marker_ids: list[str]) -> list[dict]:
        selected_ids = set(marker_ids)
        return [
            marker_snapshot(marker)
            for marker in self._project.markers
            if marker.track_id == track_id and (not selected_ids or marker.id in selected_ids)
        ]

    @staticmethod
    def _changed_marker_snapshots(before: list[dict], after: list[dict]) -> tuple[list[dict], list[dict]]:
        before_by_id = {item["id"]: item for item in before}
        after_by_id = {item["id"]: item for item in after}
        changed_ids = [
            item["id"]
            for item in before
            if before_by_id[item["id"]] != after_by_id.get(item["id"])
        ]
        changed_ids.extend(item["id"] for item in after if item["id"] not in before_by_id)
        return (
            [before_by_id[item_id] for item_id in changed_ids if item_id in before_by_id],
            [after_by_id[item_id] for item_id in changed_ids if item_id in after_by_id],
        )

    def _reconcile_selection_with_project(self) -> None:
        if self._selected_track_id and find_track(self._project, self._selected_track_id) is None:
            self._set_selected_track_id("")
            return
        if not self._selected_track_id:
            self._set_selected_marker_ids([])
            return
        marker_ids = {
            marker.id
            for marker in self._project.markers
            if marker.track_id == self._selected_track_id
        }
        self._set_selected_marker_ids(
            [marker_id for marker_id in self._selected_marker_ids if marker_id in marker_ids]
        )

    def _keep_playback_position_visible(self) -> None:
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

    def _visible_timeline_seconds(self) -> float:
        return self._timeline_visible_seconds

    def _scroll_for_visible_time(self, seconds: float) -> float:
        visible_seconds = self._visible_timeline_seconds()
        if seconds < self._timeline_scroll_seconds:
            return seconds
        if seconds > self._timeline_scroll_seconds + visible_seconds:
            return seconds - visible_seconds
        return self._timeline_scroll_seconds

    def _refresh_visible_waveforms(self, track_ids: set[str] | None = None) -> None:
        for track in self._project.tracks:
            if track_ids is not None and track.id not in track_ids:
                continue
            if track.transform_id != "waveform.summary":
                continue
            if (
                track.result_state != ResultState.COMPLETE
                or not self._has_valid_waveform_cache(track)
            ):
                if track.provenance.pop("visible_waveform", None) is not None:
                    self.trackModel.refresh_track(track.id)
                continue
            payload = track.provenance.get("waveform_payload")
            if not isinstance(payload, dict):
                if track.provenance.pop("visible_waveform", None) is not None:
                    self.trackModel.refresh_track(track.id)
                continue
            visible_waveform = self._waveform_lod.visible_samples(
                payload,
                scroll_seconds=self._timeline_scroll_seconds,
                visible_seconds=self._visible_timeline_seconds(),
                pixels_per_second=self._timeline_pixels_per_second,
            )
            if track.provenance.get("visible_waveform") != visible_waveform:
                track.provenance["visible_waveform"] = visible_waveform
                self.trackModel.refresh_track(track.id)

    def _has_valid_waveform_cache(self, track) -> bool:
        entries = {entry.id: entry for entry in self._project.cache_entries}
        return any(
            (entry := entries.get(cache_ref)) is not None
            and entry.artifact_kind == "waveform"
            and entry.validation_status == "valid"
            for cache_ref in track.cache_refs
        )

    def _load_waveform_samples(self, track_id: str) -> None:
        track = find_track(self._project, track_id)
        if track is None or track.transform_id != "waveform.summary":
            return
        if track.result_state != ResultState.COMPLETE:
            track.provenance.pop("waveform_samples", None)
            track.provenance.pop("waveform_duration_seconds", None)
            track.provenance.pop("waveform_payload", None)
            track.provenance.pop("visible_waveform", None)
            return
        entries_by_id = {entry.id: entry for entry in self._project.cache_entries}
        for cache_ref in track.cache_refs:
            entry = entries_by_id.get(cache_ref)
            if entry is None or entry.artifact_kind != "waveform" or entry.validation_status != "valid":
                continue
            artifact_path = self._job_queue.cache_store.artifact_path(entry)
            try:
                payload = json.loads(artifact_path.read_text(encoding="utf-8"))
            except (OSError, ValueError, TypeError):
                track.provenance.pop("waveform_samples", None)
                track.provenance.pop("waveform_duration_seconds", None)
                track.provenance.pop("waveform_payload", None)
                track.provenance.pop("visible_waveform", None)
                return
            samples = payload.get("samples", [])
            if isinstance(samples, list):
                track.provenance["waveform_payload"] = payload
                track.provenance["waveform_samples"] = samples
                track.provenance["waveform_duration_seconds"] = payload.get("duration", 0.0)
            else:
                track.provenance.pop("waveform_samples", None)
                track.provenance.pop("waveform_duration_seconds", None)
                track.provenance.pop("waveform_payload", None)
                track.provenance.pop("visible_waveform", None)
            return
        track.provenance.pop("waveform_samples", None)
        track.provenance.pop("waveform_duration_seconds", None)
        track.provenance.pop("waveform_payload", None)
        track.provenance.pop("visible_waveform", None)

    def _load_all_waveform_samples(self) -> None:
        for track in list(self._project.tracks):
            if track.transform_id == "waveform.summary":
                self._load_waveform_samples(track.id)

    def _attach_demo_waveform(self, waveform_track) -> None:
        samples = self._demo_waveform_samples()
        payload = {"version": 1, "duration": 1.0, "samples": samples}
        payload_bytes = json.dumps(payload).encode("utf-8")
        entry = self._job_queue.cache_store.write_bytes(
            "waveform",
            "demo-waveform",
            payload_bytes,
            "1",
        )
        waveform_track.cache_refs = [entry.id]
        waveform_track.provenance["waveform_payload"] = payload
        waveform_track.provenance["waveform_samples"] = samples
        waveform_track.provenance["waveform_duration_seconds"] = 1.0
        self._project.cache_entries.append(entry)

    @staticmethod
    def _demo_waveform_samples() -> list[dict[str, float]]:
        return [
            {
                "peak": 0.32 + 0.58 * abs(math.sin(index * 0.31)),
                "rms": 0.14 + 0.34 * abs(math.sin(index * 0.31)),
            }
            for index in range(80)
        ]

    def _marker_summary_for_track(self, track_id: str) -> list[dict]:
        selected_ids = set(self._selected_marker_ids)
        return [
            {
                "id": marker.id,
                "timestamp": marker.timestamp,
                "duration": marker.duration,
                "label": marker.label,
                "category": marker.category,
                "color": marker_display_color(marker),
                "colorKey": marker_color_key(marker),
                "selected": marker.id in selected_ids,
            }
            for marker in sorted(
                (marker for marker in self._project.markers if marker.track_id == track_id),
                key=lambda marker: (marker.timestamp, marker.id),
            )
        ]

    def _dependent_track_state_snapshots(self, track_id: str) -> list[dict]:
        dependent_ids = self._dependent_track_ids(track_id)
        return [
            {
                "index": index,
                "track": copy.deepcopy(track),
                "markers": [
                    copy.deepcopy(marker)
                    for marker in self._project.markers
                    if marker.track_id == track.id
                ],
                "job_runs": [
                    copy.deepcopy(job_run)
                    for job_run in self._project.job_runs
                    if job_run.track_id == track.id
                ],
            }
            for index, track in enumerate(self._project.tracks)
            if track.id in dependent_ids
        ]

    def _dependent_track_ids(self, track_id: str) -> set[str]:
        dependent_ids: set[str] = set()
        changed = True
        while changed:
            changed = False
            for track in self._project.tracks:
                if track.id == track_id or track.id in dependent_ids:
                    continue
                if any(input_id == track_id or input_id in dependent_ids for input_id in track.input_track_ids):
                    dependent_ids.add(track.id)
                    changed = True
        return dependent_ids

    def _editable_marker_for_selected_marker_id(self, marker_id: str) -> Marker:
        for marker in self._project.markers:
            if marker.track_id == self._selected_track_id and marker.id == marker_id:
                return marker
        raise ValueError(f"marker not found on track {self._selected_track_id}: {marker_id}")

    def _refresh_dependency_hash(self, track) -> None:
        if not track.transform_id or not track.input_track_ids:
            return
        parent = find_track(self._project, track.input_track_ids[0])
        if parent is None:
            raise ValueError(f"parent track not found: {track.input_track_ids[0]}")
        params = self._dependency_transform_params_for_track(track)
        track.dependency_hash = track_dependency_hash(
            track_dependency_inputs(self._project, parent),
            track.transform_id,
            track.transform_version,
            params,
        )

    def _submit_track(self, track_id: str) -> str:
        track = find_track(self._project, track_id)
        if track is None:
            raise ValueError(f"track not found: {track_id}")
        if not track.transform_id:
            raise ValueError("track has no transform")
        if self._active_job_id_for_track(track_id):
            raise ValueError(f"track already has a running job: {track_id}")
        incomplete_inputs = self._incomplete_input_tracks(track)
        if incomplete_inputs:
            names = ", ".join(input_track.name for input_track in incomplete_inputs)
            raise ValueError(f"input track is not complete: {names}")
        self._refresh_dependency_hash(track)
        job_id = self._job_queue.submit(
            self._project,
            track_id,
            transform_params=self._runtime_transform_params_for_track(track),
        )
        self._set_last_error("")
        self._mark_non_history_dirty()
        return job_id

    def _runtime_transform_params_for_track(self, track) -> dict:
        params = dict(track.transform_params)
        spec = self._registry.get(track.transform_id, version=track.transform_version)
        if spec.input_schema == "audio.v1":
            params.pop("audio_path", None)
            params["audio_path"] = self._require_source_audio_path_for_track(track)
        return params

    def _dependency_transform_params_for_track(self, track) -> dict:
        params = dict(track.transform_params)
        spec = self._registry.get(track.transform_id, version=track.transform_version)
        if spec.input_schema == "audio.v1":
            params.pop("audio_path", None)
        return params

    def _can_replace_project(self) -> bool:
        try:
            self._raise_if_running_jobs("replace project")
        except Exception as exc:
            self._set_last_error(str(exc))
            return False
        return True

    def _raise_if_running_jobs(self, action: str) -> None:
        if any(run.state == ResultState.RUNNING for run in self._project.job_runs):
            raise ValueError(f"cannot {action} with a running job")

    def _active_job_id_for_track(self, track_id: str) -> str:
        for run in reversed(self._project.job_runs):
            if run.track_id == track_id and run.state == ResultState.RUNNING:
                return run.id
        return ""

    def _track_inputs_are_complete(self, track) -> bool:
        return not self._incomplete_input_tracks(track)

    def _incomplete_input_tracks(self, track) -> list:
        input_tracks = []
        for input_track_id in track.input_track_ids:
            input_track = find_track(self._project, input_track_id)
            if input_track is None:
                continue
            if input_track.result_state != ResultState.COMPLETE:
                input_tracks.append(input_track)
        return input_tracks

    @staticmethod
    def _mark_running_state_stale(project) -> bool:
        changed = False
        running_track_ids = set()
        for run in project.job_runs:
            if run.state != ResultState.RUNNING:
                continue
            run.state = ResultState.STALE
            run.error = "job was running when project was opened"
            running_track_ids.add(run.track_id)
            changed = True

        for track in project.tracks:
            if track.result_state != ResultState.RUNNING and track.id not in running_track_ids:
                continue
            track.result_state = ResultState.STALE
            track.error = "job was running when project was opened"
            changed = True

        return changed

    @staticmethod
    def _path_from_qml(value: str) -> Path:
        text = str(value)
        if text.startswith("file:"):
            return Path(QUrl(text).toLocalFile())
        return Path(text)

    def __del__(self):
        try:
            self.cleanup()
        except Exception:
            pass
