from __future__ import annotations

import tempfile
from pathlib import Path

from PySide6.QtCore import Property, QObject, Slot

from autolight.project.models import Marker, ResultState
from autolight.project.store import add_generated_track, create_editable_track_from_markers, import_audio_asset, new_project
from autolight.timeline.model import TimelineTrackModel


class AppController(QObject):
    def __init__(self):
        super().__init__()
        self._project = new_project("Untitled")
        self._track_model = TimelineTrackModel()
        self._track_model.set_project(self._project)

    @Property(QObject, constant=True)
    def trackModel(self):
        return self._track_model

    @Property(str)
    def projectName(self) -> str:
        return self._project.name

    @Slot()
    def load_demo_project(self) -> None:
        tmp = Path(tempfile.gettempdir()) / "autolight-demo-song.wav"
        tmp.write_bytes(b"demo audio")
        self._project = new_project("Autolight Demo")
        source = import_audio_asset(self._project, tmp)
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
