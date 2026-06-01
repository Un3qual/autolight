from __future__ import annotations

from PySide6.QtCore import QAbstractListModel, QModelIndex, QObject, Qt, Slot

from autolight.analysis.registry import TransformRegistry, TransformSpec


class TransformSpecModel(QAbstractListModel):
    ROLE_NAMES = {
        Qt.ItemDataRole.UserRole + 1: b"transformId",
        Qt.ItemDataRole.UserRole + 2: b"version",
        Qt.ItemDataRole.UserRole + 3: b"name",
        Qt.ItemDataRole.UserRole + 4: b"estimatedCost",
        Qt.ItemDataRole.UserRole + 5: b"outputSchema",
    }

    def __init__(self, registry: TransformRegistry, parent: QObject | None = None):
        super().__init__(parent)
        self._specs: list[TransformSpec] = registry.specs()
        self._role_by_name = {
            value.decode("utf-8"): role for role, value in self.ROLE_NAMES.items()
        }

    def rowCount(self, parent: QModelIndex = QModelIndex()) -> int:
        return 0 if parent.isValid() else len(self._specs)

    def data(self, index: QModelIndex, role: int = Qt.ItemDataRole.DisplayRole):
        if not index.isValid() or index.row() < 0 or index.row() >= len(self._specs):
            return None
        spec = self._specs[index.row()]
        if role == Qt.ItemDataRole.DisplayRole or role == self.role_for_name("name"):
            return spec.name
        if role == self.role_for_name("transformId"):
            return spec.id
        if role == self.role_for_name("version"):
            return spec.version
        if role == self.role_for_name("estimatedCost"):
            return spec.estimated_cost
        if role == self.role_for_name("outputSchema"):
            return spec.output_schema
        return None

    def roleNames(self):
        return dict(self.ROLE_NAMES)

    def role_for_name(self, name: str) -> int:
        return self._role_by_name[name]

    @Slot(int, result=str)
    def version_at(self, row: int) -> str:
        if row < 0 or row >= len(self._specs):
            return ""
        return self._specs[row].version
