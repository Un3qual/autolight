from __future__ import annotations

import math

from PySide6.QtCore import QAbstractListModel, QModelIndex, QObject, Qt, Signal, Slot

from autolight.project.models import JobRun, Marker, ProjectDocument, ResultState, Track, TrackType
from autolight.project.store import marker_display_color


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
        Qt.ItemDataRole.UserRole + 14: b"waveformDurationSeconds",
        Qt.ItemDataRole.UserRole + 15: b"editable",
        Qt.ItemDataRole.UserRole + 16: b"visibleWaveformSamples",
        Qt.ItemDataRole.UserRole + 17: b"waveformLevelBucketCount",
    }

    def __init__(self, parent: QObject | None = None):
        super().__init__(parent)
        self._project: ProjectDocument | None = None
        self._markers_by_track: dict[str, list[Marker]] = {}
        self._selected_marker_ids: set[str] = set()
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
            self.role_for_name("waveformSamples"): self._waveform_samples_for_track,
            self.role_for_name("cacheRefCount"): lambda track: len(track.cache_refs),
            self.role_for_name("artifactKinds"): lambda track: ", ".join(
                self._artifact_kinds_for_track(track.cache_refs)
            ),
            self.role_for_name("waveformDurationSeconds"): self._waveform_duration_seconds_for_track,
            self.role_for_name("editable"): lambda track: track.type == TrackType.EDITABLE,
            self.role_for_name("visibleWaveformSamples"): self._visible_waveform_samples_for_track,
            self.role_for_name("waveformLevelBucketCount"): self._waveform_level_bucket_count_for_track,
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

    def set_selected_marker_ids(self, marker_ids: list[str]) -> None:
        selected_ids = set(marker_ids)
        if self._selected_marker_ids == selected_ids:
            return
        changed_ids = self._selected_marker_ids ^ selected_ids
        affected_track_ids = self._track_ids_for_marker_ids(changed_ids)
        self._selected_marker_ids = selected_ids
        for track_id in affected_track_ids:
            self.refresh_track(track_id)

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

    def _marker_spans_for_track(self, track: Track) -> list[dict[str, str | float | bool]]:
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

    def _waveform_samples_for_track(self, track: Track) -> list:
        if not self._has_complete_valid_waveform(track):
            return []
        samples = track.provenance.get("waveform_samples", [])
        return samples if isinstance(samples, list) else []

    def _waveform_duration_seconds_for_track(self, track: Track) -> float:
        if not self._has_complete_valid_waveform(track):
            return 0.0
        try:
            duration = float(track.provenance.get("waveform_duration_seconds", 0.0))
        except (TypeError, ValueError):
            return 0.0
        return duration if math.isfinite(duration) and duration >= 0.0 else 0.0

    def _visible_waveform_samples_for_track(self, track: Track) -> list:
        visible = self._visible_waveform_for_track(track)
        if visible is None:
            return []
        samples = visible.get("samples", [])
        if not isinstance(samples, list):
            return []
        return [dict(sample) for sample in samples if isinstance(sample, dict)]

    def _waveform_level_bucket_count_for_track(self, track: Track) -> int:
        visible = self._visible_waveform_for_track(track)
        if visible is None:
            return 0
        try:
            bucket_count = float(visible.get("level_bucket_count", 0))
        except (OverflowError, TypeError, ValueError):
            return 0
        if not math.isfinite(bucket_count) or bucket_count < 0:
            return 0
        return int(bucket_count)

    def _visible_waveform_for_track(self, track: Track) -> dict | None:
        if not self._has_complete_valid_waveform(track):
            return None
        visible = track.provenance.get("visible_waveform", {})
        return visible if isinstance(visible, dict) else None

    def _has_complete_valid_waveform(self, track: Track) -> bool:
        return (
            self._project is not None
            and track.result_state == ResultState.COMPLETE
            and self._has_valid_waveform_cache(track.cache_refs)
        )

    def _has_valid_waveform_cache(self, cache_refs: list[str]) -> bool:
        if self._project is None:
            return False
        entries = {entry.id: entry for entry in self._project.cache_entries}
        return any(
            (entry := entries.get(cache_ref)) is not None
            and entry.artifact_kind == "waveform"
            and entry.validation_status == "valid"
            for cache_ref in cache_refs
        )

    def _markers_for_track(self, track_id: str) -> list[Marker]:
        return self._markers_by_track.get(track_id, [])

    def _latest_job_for_track(self, track_id: str) -> JobRun | None:
        if self._project is None:
            return None
        jobs = [run for run in self._project.job_runs if run.track_id == track_id]
        return jobs[-1] if jobs else None

    def _marker_span(self, marker: Marker) -> dict[str, str | float | bool]:
        return {
            "id": marker.id,
            "timestamp": marker.timestamp,
            "duration": marker.duration or 0.0,
            "label": marker.label,
            "category": marker.category,
            "color": marker_display_color(marker),
            "selected": marker.id in self._selected_marker_ids,
        }

    def _track_ids_for_marker_ids(self, marker_ids: set[str]) -> set[str]:
        if not marker_ids:
            return set()
        return {
            marker.track_id
            for markers in self._markers_by_track.values()
            for marker in markers
            if marker.id in marker_ids
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
