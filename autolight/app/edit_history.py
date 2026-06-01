from __future__ import annotations

import copy
from dataclasses import dataclass
from typing import Any, Protocol, TYPE_CHECKING

if TYPE_CHECKING:
    from autolight.project.models import ProjectDocument


class EditCommand(Protocol):
    def undo(self, project: ProjectDocument) -> None: ...

    def redo(self, project: ProjectDocument) -> None: ...


@dataclass(slots=True)
class MarkerSnapshotCommand:
    track_id: str
    before: list[dict[str, Any]]
    after: list[dict[str, Any]]

    def __post_init__(self) -> None:
        self.before = copy.deepcopy(self.before)
        self.after = copy.deepcopy(self.after)

    def undo(self, project: ProjectDocument) -> None:
        self._restore(project, self.before)

    def redo(self, project: ProjectDocument) -> None:
        self._restore(project, self.after)

    def _restore(self, project: ProjectDocument, snapshots: list[dict[str, Any]]) -> None:
        from autolight.project.models import Marker
        from autolight.project.store import find_track, mark_dependents_stale

        affected_ids = {item["id"] for item in self.before} | {item["id"] for item in self.after}
        project.markers[:] = [
            marker
            for marker in project.markers
            if not (marker.track_id == self.track_id and marker.id in affected_ids)
        ]
        for item in snapshots:
            project.markers.append(
                Marker(
                    id=item["id"],
                    track_id=item["track_id"],
                    timestamp=item["timestamp"],
                    duration=item["duration"],
                    label=item["label"],
                    category=item["category"],
                    confidence=item["confidence"],
                    tags=list(item["tags"]),
                    source_transform=item["source_transform"],
                    source_marker_ids=list(item["source_marker_ids"]),
                    metadata=copy.deepcopy(item["metadata"]),
                )
            )
        if find_track(project, self.track_id) is not None:
            mark_dependents_stale(project, self.track_id)


@dataclass(slots=True)
class ProjectSnapshotCommand:
    before: ProjectDocument
    after: ProjectDocument

    def __post_init__(self) -> None:
        self.before = copy.deepcopy(self.before)
        self.after = copy.deepcopy(self.after)

    def undo(self, project: ProjectDocument) -> None:
        self._restore(project, self.before)

    def redo(self, project: ProjectDocument) -> None:
        self._restore(project, self.after)

    def _restore(self, project: ProjectDocument, snapshot: ProjectDocument) -> None:
        project.id = snapshot.id
        project.name = snapshot.name
        project.schema_version = snapshot.schema_version
        project.audio_assets[:] = copy.deepcopy(snapshot.audio_assets)
        project.tracks[:] = copy.deepcopy(snapshot.tracks)
        project.markers[:] = copy.deepcopy(snapshot.markers)
        project.job_runs[:] = copy.deepcopy(snapshot.job_runs)
        project.cache_entries[:] = copy.deepcopy(snapshot.cache_entries)
        project.ui_state = copy.deepcopy(snapshot.ui_state)


class EditHistory:
    def __init__(self):
        self._undo_stack: list[EditCommand] = []
        self._redo_stack: list[EditCommand] = []

    @property
    def can_undo(self) -> bool:
        return bool(self._undo_stack)

    @property
    def can_redo(self) -> bool:
        return bool(self._redo_stack)

    def push(self, command: EditCommand) -> None:
        self._undo_stack.append(command)
        self._redo_stack.clear()

    def undo(self, project: ProjectDocument) -> bool:
        if not self._undo_stack:
            return False
        command = self._undo_stack.pop()
        command.undo(project)
        self._redo_stack.append(command)
        return True

    def redo(self, project: ProjectDocument) -> bool:
        if not self._redo_stack:
            return False
        command = self._redo_stack.pop()
        command.redo(project)
        self._undo_stack.append(command)
        return True

    def clear(self) -> None:
        self._undo_stack.clear()
        self._redo_stack.clear()
