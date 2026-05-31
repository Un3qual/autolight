from __future__ import annotations

import tempfile
from pathlib import Path

from PySide6.QtCore import Property, QObject, QUrl, Signal, Slot

from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformRegistry
from autolight.cache.keys import track_dependency_hash
from autolight.jobs.queue import LocalJobQueue
from autolight.project.models import Marker, ResultState
from autolight.project.store import (
    ProjectStore,
    add_generated_track,
    create_editable_track_from_markers,
    find_track,
    import_audio_asset,
    new_project,
)
from autolight.timeline.model import TimelineTrackModel


class AppController(QObject):
    projectNameChanged = Signal()
    projectPathChanged = Signal()
    lastErrorChanged = Signal()
    selectedTrackIdChanged = Signal()
    isDirtyChanged = Signal()

    def __init__(self):
        super().__init__()
        self._project = new_project("Untitled")
        self._project_path = ""
        self._last_error = ""
        self._selected_track_id = ""
        self._is_dirty = False
        self._demo_temp_dir: tempfile.TemporaryDirectory | None = None
        self._runtime_temp_dir = tempfile.TemporaryDirectory(prefix="autolight-runtime-")
        self._track_model = TimelineTrackModel(parent=self)
        self._track_model.set_project(self._project)
        self._registry = TransformRegistry()
        register_builtin_transforms(self._registry)
        self._job_queue = LocalJobQueue(
            self._registry,
            artifact_root=Path(self._runtime_temp_dir.name) / "artifacts",
            on_track_changed=self._track_model.trackChangedRequested.emit,
        )

    @Property(QObject, constant=True)
    def trackModel(self):
        return self._track_model

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

    @Property(bool, notify=isDirtyChanged)
    def isDirty(self) -> bool:
        return self._is_dirty

    @Slot()
    def new_project(self) -> None:
        self._set_project(new_project("Untitled"))
        self._set_project_path("")
        self._set_selected_track_id("")
        self._set_last_error("")
        self._set_dirty(False)

    @Slot(str, result=bool)
    def open_project(self, path: str) -> bool:
        try:
            project_path = self._path_from_qml(path)
            project = ProjectStore.load(project_path)
            invalid_cache_refs = self._job_queue.refresh_cache_validity(project)
            changed_running_state = self._mark_running_state_stale(project)
            self._set_project(project)
            self._set_project_path(str(project_path))
            self._set_selected_track_id("")
            self._set_last_error("")
            self._set_dirty(bool(invalid_cache_refs or changed_running_state))
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
            self._raise_if_running_jobs()
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
        if self._demo_temp_dir is not None:
            self._demo_temp_dir.cleanup()
        self._demo_temp_dir = tempfile.TemporaryDirectory(prefix="autolight-demo-")
        demo_audio_name = Path(self._demo_temp_dir.name).name
        demo_audio_path = Path(self._demo_temp_dir.name) / f"{demo_audio_name}.wav"
        demo_audio_path.write_bytes(b"demo audio")

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
                parent.cache_refs,
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
            self._set_last_error("")
            self._set_dirty(True)
            return track.id
        except Exception as exc:
            self._set_last_error(str(exc))
            return ""

    @Slot(str, result=str)
    def run_track(self, track_id: str) -> str:
        try:
            job_id = self._job_queue.submit(self._project, track_id)
            self._set_last_error("")
            self._set_dirty(True)
            return job_id
        except Exception as exc:
            self._set_last_error(str(exc))
            return ""

    @Slot(str)
    def cancel_job(self, job_id: str) -> None:
        self._job_queue.cancel(job_id)

    @Slot()
    def cleanup(self) -> None:
        self._job_queue.shutdown()
        if self._demo_temp_dir is not None:
            self._demo_temp_dir.cleanup()
            self._demo_temp_dir = None
        self._runtime_temp_dir.cleanup()

    def _set_project(self, project) -> None:
        self._project = project
        self._track_model.set_project(self._project)
        self.projectNameChanged.emit()

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

    def _set_dirty(self, dirty: bool) -> None:
        if self._is_dirty == dirty:
            return
        self._is_dirty = dirty
        self.isDirtyChanged.emit()

    def _raise_if_running_jobs(self) -> None:
        if any(run.state == ResultState.RUNNING for run in self._project.job_runs):
            raise ValueError("cannot save project with a running job")

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
