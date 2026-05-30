from __future__ import annotations

from PySide6.QtCore import QAbstractListModel, QModelIndex, QObject, Qt, Signal, Slot

from autolight.project.models import Marker, ProjectDocument


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
    }

    def __init__(self, parent: QObject | None = None):
        super().__init__(parent)
        self._project: ProjectDocument | None = None
        self._markers_by_track: dict[str, list[Marker]] = {}
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

        track = self._project.tracks[row]
        if role == Qt.ItemDataRole.DisplayRole:
            return track.name
        if role == self.role_for_name("trackId"):
            return track.id
        if role == self.role_for_name("name"):
            return track.name
        if role == self.role_for_name("trackType"):
            return track.type.value
        if role == self.role_for_name("resultState"):
            return track.result_state.value
        if role == self.role_for_name("markerCount"):
            return len(self._markers_by_track.get(track.id, []))
        if role == self.role_for_name("markerSpans"):
            return [self._marker_span(marker) for marker in self._markers_for_track(track.id)]
        if role == self.role_for_name("error"):
            return track.error
        return None

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
        encoded = name.encode("utf-8")
        for role, role_name in self.ROLE_NAMES.items():
            if role_name == encoded:
                return role
        raise KeyError(name)

    def _markers_for_track(self, track_id: str) -> list[Marker]:
        return self._markers_by_track.get(track_id, [])

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
