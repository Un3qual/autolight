from __future__ import annotations

import tempfile
from pathlib import Path

from PySide6.QtCore import Property, QObject, Signal, Slot

from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformRegistry
from autolight.jobs.queue import LocalJobQueue
from autolight.project.models import Marker, ResultState
from autolight.project.store import add_generated_track, create_editable_track_from_markers, import_audio_asset, new_project
from autolight.timeline.model import TimelineTrackModel


class AppController(QObject):
    projectNameChanged = Signal()

    def __init__(self):
        super().__init__()
        self._project = new_project("Untitled")
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

    @Slot()
    def load_demo_project(self) -> None:
        if self._demo_temp_dir is not None:
            self._demo_temp_dir.cleanup()
        self._demo_temp_dir = tempfile.TemporaryDirectory(prefix="autolight-demo-")
        demo_audio_name = Path(self._demo_temp_dir.name).name
        demo_audio_path = Path(self._demo_temp_dir.name) / f"{demo_audio_name}.wav"
        demo_audio_path.write_bytes(b"demo audio")

        self._project = new_project("Autolight Demo")
        self.projectNameChanged.emit()
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

    @Slot(str, result=str)
    def run_track(self, track_id: str) -> str:
        return self._job_queue.submit(self._project, track_id)

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

    def __del__(self):
        try:
            self.cleanup()
        except Exception:
            pass
