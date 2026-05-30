from __future__ import annotations

from PySide6.QtCore import QAbstractListModel, QModelIndex, QObject, Qt

from autolight.project.models import ProjectDocument


class TimelineTrackModel(QAbstractListModel):
    ROLE_NAMES = {
        Qt.ItemDataRole.UserRole + 1: b"trackId",
        Qt.ItemDataRole.UserRole + 2: b"name",
        Qt.ItemDataRole.UserRole + 3: b"trackType",
        Qt.ItemDataRole.UserRole + 4: b"resultState",
        Qt.ItemDataRole.UserRole + 5: b"markerCount",
        Qt.ItemDataRole.UserRole + 6: b"error",
    }

    def __init__(self, parent: QObject | None = None):
        super().__init__(parent)
        self._project: ProjectDocument | None = None

    def set_project(self, project: ProjectDocument) -> None:
        self.beginResetModel()
        self._project = project
        self.endResetModel()

    def rowCount(self, parent: QModelIndex = QModelIndex()) -> int:
        if parent.isValid() or self._project is None:
            return 0
        return len(self._project.tracks)

    def data(self, index: QModelIndex, role: int = Qt.ItemDataRole.DisplayRole):
        if (
            self._project is None
            or not index.isValid()
            or index.model() is not self
            or index.column() != 0
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
            return len([marker for marker in self._project.markers if marker.track_id == track.id])
        if role == self.role_for_name("error"):
            return track.error
        return None

    def roleNames(self):
        return dict(self.ROLE_NAMES)

    def role_for_name(self, name: str) -> int:
        encoded = name.encode("utf-8")
        for role, role_name in self.ROLE_NAMES.items():
            if role_name == encoded:
                return role
        raise KeyError(name)
