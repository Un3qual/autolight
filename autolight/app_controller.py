from __future__ import annotations

import json
import math
import tempfile
from pathlib import Path

from PySide6.QtCore import Property, QObject, Qt, QUrl, Signal, Slot

from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformRegistry
from autolight.cache.keys import track_dependency_hash
from autolight.demo_audio import write_silent_wav
from autolight.jobs.queue import LocalJobQueue
from autolight.playback import PlaybackTransport
from autolight.project.models import AudioAsset, Marker, ResultState, TrackType
from autolight.project.store import (
    ProjectStore,
    add_editable_marker,
    add_generated_track,
    create_editable_track_from_markers,
    delete_editable_marker,
    find_track,
    import_audio_asset,
    new_project,
    refresh_audio_asset_status,
    refresh_audio_track_status,
    track_dependency_inputs,
)
from autolight.timeline.model import TimelineTrackModel
from autolight.timeline.transform_model import TransformSpecModel


class AppController(QObject):
    projectNameChanged = Signal()
    projectPathChanged = Signal()
    lastErrorChanged = Signal()
    selectedTrackIdChanged = Signal()
    selectedTrackMarkersChanged = Signal()
    selectedTrackHasRunningJobChanged = Signal()
    selectedTrackCanRerunChanged = Signal()
    selectedTrackCanPlayChanged = Signal()
    timelineDurationSecondsChanged = Signal()
    timelinePixelsPerSecondChanged = Signal()
    timelineScrollSecondsChanged = Signal()
    isDirtyChanged = Signal()
    _track_changed_on_main_thread = Signal(str)

    def __init__(self):
        super().__init__()
        self._project = new_project("Untitled")
        self._project_path = ""
        self._last_error = ""
        self._selected_track_id = ""
        self._is_dirty = False
        self._demo_temp_dir: tempfile.TemporaryDirectory | None = None
        self._runtime_temp_dir = tempfile.TemporaryDirectory(prefix="autolight-runtime-")
        self._playback = PlaybackTransport(parent=self)
        self._timeline_pixels_per_second = 96.0
        self._timeline_scroll_seconds = 0.0
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

    @Property(str, notify=projectNameChanged)
    def projectName(self) -> str:
        return self._project.name

    @Property(str, notify=projectPathChanged)
    def projectPath(self) -> str:
        return self._project_path

    @Property(str, notify=lastErrorChanged)
    def lastError(self) -> str:
        return self._last_error

    @Property(str, notify=selectedTrackIdChanged)
    def selectedTrackId(self) -> str:
        return self._selected_track_id

    @Property(list, notify=selectedTrackMarkersChanged)
    def selectedTrackMarkers(self) -> list[dict]:
        return self._marker_summary_for_track(self._selected_track_id)

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
        return self._is_dirty

    @Slot()
    def new_project(self) -> None:
        if not self._can_replace_project():
            return
        self._set_project(new_project("Untitled"))
        self._set_project_path("")
        self._set_selected_track_id("")
        self._set_last_error("")
        self._set_dirty(False)

    @Slot(str, result=bool)
    def open_project(self, path: str) -> bool:
        try:
            self._raise_if_running_jobs("replace project")
            project_path = self._path_from_qml(path)
            project = ProjectStore.load(project_path)
            changed_running_state = self._mark_running_state_stale(project)
            changed_audio_asset_ids = refresh_audio_asset_status(project, search_dirs=[project_path.parent])
            changed_audio_track_ids = refresh_audio_track_status(project)
            self._set_project(project)
            self._set_project_path(str(project_path))
            self._set_selected_track_id("")
            self._set_last_error("")
            invalid_cache_refs = self.refresh_cache_status()
            self.selectedTrackCanRerunChanged.emit()
            self._set_dirty(
                bool(
                    invalid_cache_refs
                    or changed_running_state
                    or changed_audio_asset_ids
                    or changed_audio_track_ids
                )
            )
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
            ProjectStore.save(self._project, project_path)
            self._set_project_path(str(project_path))
            self._set_last_error("")
            self._set_dirty(False)
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
            self._set_dirty(True)
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
                Marker(id="marker_demo_1", track_id=beats.id, timestamp=0.0, label="Beat"),
                Marker(id="marker_demo_2", track_id=beats.id, timestamp=0.5, label="Beat"),
                Marker(id="marker_demo_3", track_id=beats.id, timestamp=1.0, label="Beat"),
            ]
        )
        create_editable_track_from_markers(self._project, beats.id, "Editable Cues", ["marker_demo_1", "marker_demo_2"])
        self._track_model.set_project(self._project)
        self._set_selected_track_id(source.id)
        self._notify_timeline_duration_changed()
        self._set_last_error("")
        self._set_dirty(False)

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
            self._set_dirty(True)
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
            self._set_dirty(True)
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
            self._set_dirty(True)
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
            self._set_dirty(True)
            return track.id
        except Exception as exc:
            self._set_last_error(str(exc))
            return ""

    @Slot(float, str, result=str)
    def add_marker_to_selected_track(self, timestamp: float, label: str) -> str:
        try:
            marker = add_editable_marker(self._project, self._selected_track_id, timestamp, label)
            self._track_model.set_project(self._project)
            self.selectedTrackMarkersChanged.emit()
            self._notify_timeline_duration_changed()
            self._set_last_error("")
            self._set_dirty(True)
            return marker.id
        except Exception as exc:
            self._set_last_error(str(exc))
            return ""

    @Slot(str, result=bool)
    def delete_marker_from_selected_track(self, marker_id: str) -> bool:
        try:
            deleted = delete_editable_marker(self._project, self._selected_track_id, marker_id)
            self._track_model.set_project(self._project)
            self.selectedTrackMarkersChanged.emit()
            self._set_last_error("")
            if deleted:
                self._notify_timeline_duration_changed()
                self._set_dirty(True)
            return deleted
        except Exception as exc:
            self._set_last_error(str(exc))
            return False

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
            self._track_model.set_project(self._project)
            self.selectedTrackCanRerunChanged.emit()
            if invalid_refs:
                self._set_dirty(True)
                self._set_last_error(f"invalid cache artifacts: {len(invalid_refs)}")
            else:
                self._set_last_error("")
            return invalid_refs
        except Exception as exc:
            self._set_last_error(str(exc))
            return []

    @Slot(result=bool)
    def play_selected_track(self) -> bool:
        asset = self._source_audio_asset_for_track_id(self._selected_track_id)
        if asset is None:
            self._set_last_error("selected track has no source audio")
            return False
        if asset.import_status != "online":
            self._set_last_error(f"source audio is {asset.import_status}")
            return False
        if self._playback.sourcePath != asset.path:
            if not self._playback.load_source(asset.path, asset.duration):
                self._set_last_error(self._playback.lastError)
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
        self.set_timeline_scroll_seconds(self._scroll_for_visible_time(seconds))

    @Slot(float)
    def set_timeline_zoom(self, pixels_per_second: float) -> None:
        value = float(pixels_per_second)
        if not math.isfinite(value):
            return
        clamped = min(max(value, 24.0), 240.0)
        if self._timeline_pixels_per_second == clamped:
            return
        self._timeline_pixels_per_second = clamped
        self.timelinePixelsPerSecondChanged.emit()
        self.set_timeline_scroll_seconds(self._timeline_scroll_seconds)

    @Slot(float)
    def set_timeline_scroll_seconds(self, seconds: float) -> None:
        value = float(seconds)
        if not math.isfinite(value):
            return
        duration = self._timeline_duration_seconds()
        visible_seconds = self._visible_timeline_seconds()
        maximum = max(0.0, duration - visible_seconds)
        clamped = min(max(value, 0.0), maximum)
        if self._timeline_scroll_seconds == clamped:
            return
        self._timeline_scroll_seconds = clamped
        self.timelineScrollSecondsChanged.emit()

    @Slot()
    def cleanup(self) -> None:
        self._job_queue.shutdown()
        if self._demo_temp_dir is not None:
            self._demo_temp_dir.cleanup()
            self._demo_temp_dir = None
        self._runtime_temp_dir.cleanup()

    def _set_project(self, project) -> None:
        self._playback.unload()
        self._project = project
        self._load_all_waveform_samples()
        self._track_model.set_project(self._project)
        self._timeline_scroll_seconds = 0.0
        self.projectNameChanged.emit()
        self.selectedTrackCanRerunChanged.emit()
        self.selectedTrackCanPlayChanged.emit()
        self._notify_timeline_duration_changed()
        self.timelineScrollSecondsChanged.emit()

    def _set_project_path(self, path: str) -> None:
        if self._project_path == path:
            return
        self._project_path = path
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
        self.selectedTrackIdChanged.emit()
        self.selectedTrackMarkersChanged.emit()
        self.selectedTrackHasRunningJobChanged.emit()
        self.selectedTrackCanRerunChanged.emit()
        self.selectedTrackCanPlayChanged.emit()

    def _set_dirty(self, dirty: bool) -> None:
        if self._is_dirty == dirty:
            return
        self._is_dirty = dirty
        self.isDirtyChanged.emit()

    def _queue_track_changed(self, track_id: str) -> None:
        self._track_changed_on_main_thread.emit(track_id)

    @Slot(str)
    def _handle_track_changed(self, track_id: str) -> None:
        self._load_waveform_samples(track_id)
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

    def _source_audio_asset_for_track_id(self, track_id: str) -> AudioAsset | None:
        track = find_track(self._project, track_id)
        if track is None:
            return None
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

    def _visible_timeline_seconds(self) -> float:
        return 8.0

    def _scroll_for_visible_time(self, seconds: float) -> float:
        duration = self._timeline_duration_seconds()
        visible_seconds = self._visible_timeline_seconds()
        if seconds < self._timeline_scroll_seconds:
            return seconds
        if seconds > self._timeline_scroll_seconds + visible_seconds:
            return seconds - visible_seconds
        return self._timeline_scroll_seconds

    def _load_waveform_samples(self, track_id: str) -> None:
        track = find_track(self._project, track_id)
        if track is None or track.transform_id != "waveform.summary":
            return
        if track.result_state != ResultState.COMPLETE:
            track.provenance.pop("waveform_samples", None)
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
                return
            samples = payload.get("samples", [])
            if isinstance(samples, list):
                track.provenance["waveform_samples"] = samples
            else:
                track.provenance.pop("waveform_samples", None)
            return
        track.provenance.pop("waveform_samples", None)

    def _load_all_waveform_samples(self) -> None:
        for track in list(self._project.tracks):
            if track.transform_id == "waveform.summary":
                self._load_waveform_samples(track.id)

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
        self._set_dirty(True)
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
