from __future__ import annotations

from PySide6.QtCore import QAbstractListModel, QModelIndex, QObject, Qt, Signal, Slot

from autolight.project.models import JobRun, Marker, ProjectDocument, Track


class TimelineTrackModel(QAbstractListModel):
    trackChangedRequested = Signal(str)

    ROLE_NAMES = {
        Qt.ItemDataRole.UserRole + 1: b"trackId",
        Qt.ItemDataRole.UserRole + 2: b"name",
        Qt.ItemDataRole.UserRole + 3: b"trackType",
        Qt.ItemDataRole.UserRole + 4: b"resultState",
        Qt.ItemDataRole.UserRole + 5: b"markerCount",
        Qt.ItemDataRole.UserRole + 6: b"markerSpans",
        Qt.ItemDataRole.UserRole + 7: b"error",
        Qt.ItemDataRole.UserRole + 8: b"activeJobId",
        Qt.ItemDataRole.UserRole + 9: b"jobState",
        Qt.ItemDataRole.UserRole + 10: b"jobProgress",
        Qt.ItemDataRole.UserRole + 11: b"waveformSamples",
        Qt.ItemDataRole.UserRole + 12: b"cacheRefCount",
        Qt.ItemDataRole.UserRole + 13: b"artifactKinds",
    }

    def __init__(self, parent: QObject | None = None):
        super().__init__(parent)
        self._project: ProjectDocument | None = None
        self._markers_by_track: dict[str, list[Marker]] = {}
        self._role_by_name = {
            role_name.decode("utf-8"): role for role, role_name in self.ROLE_NAMES.items()
        }
        self._role_handlers = {
            self.role_for_name("trackId"): lambda track: track.id,
            self.role_for_name("name"): lambda track: track.name,
            self.role_for_name("trackType"): lambda track: track.type.value,
            self.role_for_name("resultState"): lambda track: track.result_state.value,
            self.role_for_name("markerCount"): self._marker_count_for_track,
            self.role_for_name("markerSpans"): self._marker_spans_for_track,
            self.role_for_name("error"): lambda track: track.error,
            self.role_for_name("activeJobId"): self._active_job_id_for_track,
            self.role_for_name("jobState"): self._job_state_for_track,
            self.role_for_name("jobProgress"): self._job_progress_for_track,
            self.role_for_name("waveformSamples"): lambda track: track.provenance.get("waveform_samples", []),
            self.role_for_name("cacheRefCount"): lambda track: len(track.cache_refs),
            self.role_for_name("artifactKinds"): lambda track: ", ".join(
                self._artifact_kinds_for_track(track.cache_refs)
            ),
        }
        self._generation = 0
        self.trackChangedRequested.connect(self.refresh_track)

    def set_project(self, project: ProjectDocument) -> None:
        self.beginResetModel()
        self._project = project
        self._rebuild_marker_index()
        self._generation += 1
        self.endResetModel()

    def rowCount(self, parent: QModelIndex = QModelIndex()) -> int:
        if parent.isValid() or self._project is None:
            return 0
        return len(self._project.tracks)

    def index(self, row: int, column: int, parent: QModelIndex = QModelIndex()) -> QModelIndex:
        if (
            self._project is None
            or parent.isValid()
            or column != 0
            or row < 0
            or row >= len(self._project.tracks)
        ):
            return QModelIndex()
        return self.createIndex(row, column, self._generation)

    def data(self, index: QModelIndex, role: int = Qt.ItemDataRole.DisplayRole):
        track = self._track_for_index(index)
        if track is None:
            return None
        if role == Qt.ItemDataRole.DisplayRole:
            return track.name
        handler = self._role_handlers.get(role)
        return None if handler is None else handler(track)

    def roleNames(self):
        return dict(self.ROLE_NAMES)

    @Slot(str)
    def refresh_track(self, track_id: str) -> None:
        if self._project is None:
            return
        row = next(
            (index for index, track in enumerate(self._project.tracks) if track.id == track_id),
            None,
        )
        if row is None:
            self._markers_by_track.pop(track_id, None)
            return
        self._rebuild_marker_index_for_track(track_id)
        model_index = self.index(row, 0)
        if model_index.isValid():
            self.dataChanged.emit(model_index, model_index, list(self.ROLE_NAMES))

    def role_for_name(self, name: str) -> int:
        return self._role_by_name[name]

    def _track_for_index(self, index: QModelIndex) -> Track | None:
        if (
            self._project is None
            or not index.isValid()
            or index.model() is not self
            or index.column() != 0
            or index.internalId() != self._generation
        ):
            return None
        row = index.row()
        if row < 0 or row >= len(self._project.tracks):
            return None
        return self._project.tracks[row]

    def _marker_count_for_track(self, track: Track) -> int:
        return len(self._markers_by_track.get(track.id, []))

    def _marker_spans_for_track(self, track: Track) -> list[dict[str, str | float]]:
        return [self._marker_span(marker) for marker in self._markers_for_track(track.id)]

    def _active_job_id_for_track(self, track: Track) -> str:
        latest_job = self._latest_job_for_track(track.id)
        return "" if latest_job is None or latest_job.state.value != "running" else latest_job.id

    def _job_state_for_track(self, track: Track) -> str:
        latest_job = self._latest_job_for_track(track.id)
        return "" if latest_job is None else latest_job.state.value

    def _job_progress_for_track(self, track: Track) -> float:
        latest_job = self._latest_job_for_track(track.id)
        return 0.0 if latest_job is None else latest_job.progress

    def _artifact_kinds_for_track(self, cache_refs: list[str]) -> list[str]:
        if self._project is None:
            return []
        entries = {entry.id: entry for entry in self._project.cache_entries}
        return [
            entries[cache_ref].artifact_kind
            for cache_ref in cache_refs
            if cache_ref in entries
        ]

    def _markers_for_track(self, track_id: str) -> list[Marker]:
        return self._markers_by_track.get(track_id, [])

    def _latest_job_for_track(self, track_id: str) -> JobRun | None:
        if self._project is None:
            return None
        jobs = [run for run in self._project.job_runs if run.track_id == track_id]
        return jobs[-1] if jobs else None

    def _marker_span(self, marker: Marker) -> dict[str, str | float]:
        return {
            "id": marker.id,
            "timestamp": marker.timestamp,
            "duration": marker.duration or 0.0,
            "label": marker.label,
            "category": marker.category,
        }

    def _rebuild_marker_index(self) -> None:
        self._markers_by_track = {}
        if self._project is None:
            return
        for marker in self._project.markers:
            self._markers_by_track.setdefault(marker.track_id, []).append(marker)
        for markers in self._markers_by_track.values():
            markers.sort(key=lambda marker: (marker.timestamp, marker.id))

    def _rebuild_marker_index_for_track(self, track_id: str) -> None:
        if self._project is None:
            self._markers_by_track = {}
            return
        self._markers_by_track[track_id] = sorted(
            (marker for marker in self._project.markers if marker.track_id == track_id),
            key=lambda marker: (marker.timestamp, marker.id),
        )
