from __future__ import annotations

import copy
from dataclasses import dataclass, field
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
class TrackSnapshotCommand:
    track_id: str
    before: Any | None
    after: Any | None
    index: int
    before_markers: list[Any] = field(default_factory=list)
    after_markers: list[Any] = field(default_factory=list)
    before_job_runs: list[Any] = field(default_factory=list)
    after_job_runs: list[Any] = field(default_factory=list)

    def __post_init__(self) -> None:
        self.before = copy.deepcopy(self.before)
        self.after = copy.deepcopy(self.after)
        self.before_markers = copy.deepcopy(self.before_markers)
        self.after_markers = copy.deepcopy(self.after_markers)
        self.before_job_runs = copy.deepcopy(self.before_job_runs)
        self.after_job_runs = copy.deepcopy(self.after_job_runs)

    def undo(self, project: ProjectDocument) -> None:
        self._restore(project, self.before, self.before_markers, self.before_job_runs)

    def redo(self, project: ProjectDocument) -> None:
        self._restore(project, self.after, self.after_markers, self.after_job_runs)

    def _restore(
        self,
        project: ProjectDocument,
        snapshot: Any | None,
        markers: list[Any],
        job_runs: list[Any],
    ) -> None:
        if snapshot is None:
            dependent = next(
                (
                    track
                    for track in project.tracks
                    if track.id != self.track_id and self.track_id in track.input_track_ids
                ),
                None,
            )
            if dependent is not None:
                raise ValueError(f"cannot remove track with dependent track: {dependent.id}")
            project.tracks[:] = [track for track in project.tracks if track.id != self.track_id]
            project.markers[:] = [marker for marker in project.markers if marker.track_id != self.track_id]
            project.job_runs[:] = [job for job in project.job_runs if job.track_id != self.track_id]
            return

        project.tracks[:] = [track for track in project.tracks if track.id != self.track_id]
        project.markers[:] = [marker for marker in project.markers if marker.track_id != self.track_id]
        project.job_runs[:] = [job for job in project.job_runs if job.track_id != self.track_id]
        insert_at = min(max(0, self.index), len(project.tracks))
        project.tracks.insert(insert_at, copy.deepcopy(snapshot))
        project.markers.extend(copy.deepcopy(markers))
        project.job_runs.extend(copy.deepcopy(job_runs))


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

    @staticmethod
    def _restore(project: ProjectDocument, snapshot: ProjectDocument) -> None:
        project.id = snapshot.id
        project.name = snapshot.name
        project.schema_version = snapshot.schema_version
        project.audio_assets[:] = copy.deepcopy(snapshot.audio_assets)
        project.tracks[:] = copy.deepcopy(snapshot.tracks)
        project.markers[:] = copy.deepcopy(snapshot.markers)
        project.job_runs[:] = copy.deepcopy(snapshot.job_runs)
        project.cache_entries[:] = copy.deepcopy(snapshot.cache_entries)
        project.ui_state.clear()
        project.ui_state.update(copy.deepcopy(snapshot.ui_state))


class EditHistory:
    def __init__(self):
        self._undo_stack: list[EditCommand] = []
        self._redo_stack: list[EditCommand] = []
        self._clean_undo_depth: int | None = 0

    @property
    def can_undo(self) -> bool:
        return bool(self._undo_stack)

    @property
    def can_redo(self) -> bool:
        return bool(self._redo_stack)

    def push(self, command: EditCommand) -> None:
        if (
            self._clean_undo_depth is not None
            and self._redo_stack
            and self._clean_undo_depth > len(self._undo_stack)
        ):
            self._clean_undo_depth = None
        self._undo_stack.append(command)
        self._redo_stack.clear()

    def undo(self, project: ProjectDocument) -> bool:
        if not self._undo_stack:
            return False
        command = self._undo_stack.pop()
        try:
            command.undo(project)
        except Exception:
            self._undo_stack.append(command)
            raise
        self._redo_stack.append(command)
        return True

    def redo(self, project: ProjectDocument) -> bool:
        if not self._redo_stack:
            return False
        command = self._redo_stack.pop()
        try:
            command.redo(project)
        except Exception:
            self._redo_stack.append(command)
            raise
        self._undo_stack.append(command)
        return True

    def clear(self) -> None:
        self._undo_stack.clear()
        self._redo_stack.clear()
        self._clean_undo_depth = 0

    def mark_clean(self) -> None:
        self._clean_undo_depth = len(self._undo_stack)

    def is_clean(self) -> bool:
        return self._clean_undo_depth == len(self._undo_stack)
