from __future__ import annotations

import copy
import shutil
from concurrent.futures import Future, ThreadPoolExecutor
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from threading import Event, Lock
from typing import Any, Callable

from autolight.analysis.registry import TransformCancelled, TransformContext, TransformRegistry, TransformResult
from autolight.cache.store import CacheStore
from autolight.project.models import CacheEntry, JobRun, Marker, ProjectDocument, ResultState, Track
from autolight.project.store import find_track, mark_dependents_stale, new_id


@dataclass(frozen=True, slots=True)
class _JobSnapshot:
    track_id: str
    transform_id: str
    transform_version: str
    transform_params: dict[str, Any]
    dependency_hash: str


class LocalJobQueue:
    def __init__(
        self,
        registry: TransformRegistry,
        artifact_root: Path,
        on_track_changed: Callable[[str], None] | None = None,
    ):
        self.registry = registry
        self.artifact_root = artifact_root
        self.artifact_root.mkdir(parents=True, exist_ok=True)
        self.cache_store = CacheStore(self.artifact_root / "cache")
        self._work_root = self.artifact_root / "work"
        self._work_root.mkdir(parents=True, exist_ok=True)
        self._on_track_changed = on_track_changed
        self._executor = ThreadPoolExecutor(max_workers=2)
        self._lock = Lock()
        self._futures: dict[str, Future] = {}
        self._cancel_events: dict[str, Event] = {}
        self._active_job_by_track: dict[str, str] = {}

    def submit(self, project: ProjectDocument, track_id: str) -> str:
        with self._lock:
            track = find_track(project, track_id)
            if track is None:
                raise ValueError(f"track not found: {track_id}")
            if not track.transform_id:
                raise ValueError("track has no transform")

            active_job_id = self._active_job_by_track.get(track_id)
            active_future = self._futures.get(active_job_id) if active_job_id is not None else None
            if active_job_id is not None and (active_future is None or not active_future.done()):
                raise ValueError(f"track already has a running job: {track_id}")
            if active_job_id is not None:
                self._active_job_by_track.pop(track_id, None)

            job_id = new_id("job")
            cancel_event = Event()
            snapshot = _JobSnapshot(
                track_id=track_id,
                transform_id=track.transform_id,
                transform_version=track.transform_version,
                transform_params=copy.deepcopy(track.transform_params),
                dependency_hash=track.dependency_hash,
            )
            run = JobRun(
                id=job_id,
                track_id=track_id,
                transform_id=snapshot.transform_id,
                parameters_hash=snapshot.dependency_hash,
                state=ResultState.RUNNING,
                started_at=datetime.now(timezone.utc).isoformat(),
            )
            project.job_runs.append(run)
            track.result_state = ResultState.RUNNING
            track.error = ""
            self._active_job_by_track[track_id] = job_id

            future = self._executor.submit(self._run, project, track, run, cancel_event, snapshot)
            self._futures[job_id] = future
            self._cancel_events[job_id] = cancel_event

        def cleanup_completed(completed_future: Future, completed_job_id: str = job_id) -> None:
            self._cleanup_job(completed_job_id, completed_future)

        future.add_done_callback(cleanup_completed)
        self._notify_track_changed(track_id)
        return job_id

    def cancel(self, job_id: str) -> None:
        with self._lock:
            event = self._cancel_events.get(job_id)
            if event is not None:
                event.set()

    def wait(self, job_id: str, timeout: float | None = None) -> None:
        with self._lock:
            future = self._futures.get(job_id)
        if future is None:
            return
        try:
            future.result(timeout=timeout)
        finally:
            if future.done():
                self._cleanup_job(job_id, future)

    def shutdown(self) -> None:
        self._executor.shutdown(wait=True)

    def refresh_cache_validity(self, project: ProjectDocument) -> list[str]:
        invalid_refs: list[str] = []
        changed_track_ids: list[str] = []
        with self._lock:
            entries_by_id = {entry.id: entry for entry in project.cache_entries}
            for entry in project.cache_entries:
                if self.cache_store.is_entry_valid(entry):
                    entry.validation_status = "valid"
                else:
                    entry.validation_status = "invalid"

            for track in project.tracks:
                track_invalid_refs = [
                    cache_ref
                    for cache_ref in track.cache_refs
                    if cache_ref not in entries_by_id
                    or entries_by_id[cache_ref].validation_status != "valid"
                ]
                if not track_invalid_refs:
                    continue
                invalid_refs.extend(track_invalid_refs)
                if track.result_state == ResultState.COMPLETE:
                    track.result_state = ResultState.STALE
                track.error = f"cache artifact missing or invalid: {track_invalid_refs[0]}"
                changed_track_ids.append(track.id)

        for changed_track_id in changed_track_ids:
            self._notify_track_changed(changed_track_id)
        return invalid_refs

    def _run(
        self,
        project: ProjectDocument,
        track: Track,
        run: JobRun,
        cancel_event: Event,
        snapshot: _JobSnapshot,
    ) -> None:
        artifact_dir = self._work_root / run.id
        additional_changed_track_ids: list[str] = []

        try:
            result = self._execute_transform(
                artifact_dir,
                cancel_event,
                snapshot,
                self._progress_callback(run, snapshot),
            )
            additional_changed_track_ids = self._handle_transform_result(
                project,
                track,
                run,
                cancel_event,
                snapshot,
                result,
            )
        except TransformCancelled:
            self._mark_finished(track, run, ResultState.CANCELLED, snapshot)
        except Exception as exc:
            self._mark_finished(track, run, ResultState.FAILED, snapshot, error=str(exc))
        finally:
            self._finish_run(track, run, artifact_dir, additional_changed_track_ids)

    def _progress_callback(self, run: JobRun, snapshot: _JobSnapshot) -> Callable[[float], None]:
        last_notified_progress = 0.0

        def progress(value: float) -> None:
            nonlocal last_notified_progress
            should_notify = False
            with self._lock:
                run.progress = max(0.0, min(1.0, value))
                if run.progress >= 1.0 or run.progress - last_notified_progress >= 0.05:
                    last_notified_progress = run.progress
                    should_notify = True
            if should_notify:
                self._notify_track_changed(snapshot.track_id)

        return progress

    def _execute_transform(
        self,
        artifact_dir: Path,
        cancel_event: Event,
        snapshot: _JobSnapshot,
        progress: Callable[[float], None],
    ) -> TransformResult:
        transform = self.registry.get(snapshot.transform_id, version=snapshot.transform_version)
        artifact_dir.mkdir(parents=True, exist_ok=True)
        context = TransformContext(
            artifact_dir=artifact_dir,
            cancel_requested=cancel_event.is_set,
            progress=progress,
        )
        return transform.run(context, snapshot.transform_params)

    def _handle_transform_result(
        self,
        project: ProjectDocument,
        track: Track,
        run: JobRun,
        cancel_event: Event,
        snapshot: _JobSnapshot,
        result: TransformResult,
    ) -> list[str]:
        if cancel_event.is_set():
            self._mark_finished(track, run, ResultState.CANCELLED, snapshot)
            return []
        markers = [
            self._marker_from_result(snapshot.track_id, snapshot.transform_id, item)
            for item in result.markers
        ]
        cache_entries = self._cache_entries_from_artifacts(result.artifacts, snapshot)
        with self._lock:
            if cancel_event.is_set():
                self._mark_finished_locked(track, run, ResultState.CANCELLED, snapshot)
                return []
            return self._commit_transform_result_locked(project, track, run, snapshot, markers, cache_entries)

    def _commit_transform_result_locked(
        self,
        project: ProjectDocument,
        track: Track,
        run: JobRun,
        snapshot: _JobSnapshot,
        markers: list[Marker],
        cache_entries: list[CacheEntry],
    ) -> list[str]:
        if not self._can_commit_locked(track, run, snapshot):
            self._mark_stale_run_locked(run)
            return []
        cache_refs = [entry.id for entry in cache_entries]
        run.state = ResultState.COMPLETE
        run.progress = 1.0
        run.produced_cache_refs = cache_refs
        self._upsert_cache_entries_locked(project, cache_entries)
        self._replace_track_markers_locked(project, track, markers)
        track.cache_refs = cache_refs
        track.result_state = ResultState.COMPLETE
        track.error = ""
        return self._mark_dependents_stale_locked(project, track.id)

    @staticmethod
    def _replace_track_markers_locked(project: ProjectDocument, track: Track, markers: list[Marker]) -> None:
        replacement_markers = [marker for marker in project.markers if marker.track_id != track.id]
        replacement_markers.extend(markers)
        project.markers[:] = replacement_markers

    @staticmethod
    def _mark_dependents_stale_locked(project: ProjectDocument, track_id: str) -> list[str]:
        previous_states = {candidate.id: candidate.result_state for candidate in project.tracks}
        mark_dependents_stale(project, track_id)
        return [
            candidate.id
            for candidate in project.tracks
            if candidate.id != track_id and candidate.result_state != previous_states[candidate.id]
        ]

    def _finish_run(
        self,
        track: Track,
        run: JobRun,
        artifact_dir: Path,
        additional_changed_track_ids: list[str],
    ) -> None:
        with self._lock:
            run.completed_at = datetime.now(timezone.utc).isoformat()
            if self._active_job_by_track.get(track.id) == run.id:
                self._active_job_by_track.pop(track.id, None)
        shutil.rmtree(artifact_dir, ignore_errors=True)
        self._notify_changed_tracks(track.id, additional_changed_track_ids)

    def _notify_changed_tracks(self, track_id: str, additional_track_ids: list[str]) -> None:
        notified_track_ids = {track_id}
        self._notify_track_changed(track_id)
        for changed_track_id in additional_track_ids:
            if changed_track_id not in notified_track_ids:
                notified_track_ids.add(changed_track_id)
                self._notify_track_changed(changed_track_id)

    def _mark_finished(
        self,
        track: Track,
        run: JobRun,
        state: ResultState,
        snapshot: _JobSnapshot,
        error: str = "",
    ) -> None:
        with self._lock:
            self._mark_finished_locked(track, run, state, snapshot, error=error)

    def _mark_finished_locked(
        self,
        track: Track,
        run: JobRun,
        state: ResultState,
        snapshot: _JobSnapshot,
        error: str = "",
    ) -> None:
        if self._can_commit_locked(track, run, snapshot):
            run.state = state
            run.error = error
            track.result_state = state
            track.error = error
        else:
            self._mark_stale_run_locked(run)

    def _mark_stale_run_locked(self, run: JobRun) -> None:
        run.state = ResultState.STALE
        run.error = "track changed while job was running"

    def _can_commit_locked(self, track: Track, run: JobRun, snapshot: _JobSnapshot) -> bool:
        return (
            self._active_job_by_track.get(track.id) == run.id
            and track.result_state == ResultState.RUNNING
            and track.transform_id == snapshot.transform_id
            and track.transform_version == snapshot.transform_version
            and track.transform_params == snapshot.transform_params
            and track.dependency_hash == snapshot.dependency_hash
        )

    def _marker_from_result(self, track_id: str, transform_id: str, item: dict) -> Marker:
        if not isinstance(item, dict):
            raise ValueError("marker result must be a dict")

        try:
            timestamp = float(item["timestamp"])
        except KeyError as exc:
            raise ValueError("marker missing timestamp") from exc

        duration_value = item.get("duration")
        confidence_value = item.get("confidence")
        tags = item.get("tags", [])
        source_marker_ids = item.get("source_marker_ids", [])
        metadata = item.get("metadata", {})
        source_transform = item.get("source_transform", transform_id)

        if tags is None:
            tags = []
        if source_marker_ids is None:
            source_marker_ids = []
        if metadata is None:
            metadata = {}
        if source_transform is None:
            source_transform = transform_id
        if not isinstance(tags, list):
            raise ValueError("marker tags must be a list")
        if not isinstance(source_marker_ids, list):
            raise ValueError("marker source_marker_ids must be a list")
        if not isinstance(metadata, dict):
            raise ValueError("marker metadata must be a dict")

        return Marker(
            id=new_id("marker"),
            track_id=track_id,
            timestamp=timestamp,
            duration=None if duration_value is None else float(duration_value),
            label=str(item.get("label", "")),
            category=str(item.get("category", "")),
            confidence=None if confidence_value is None else float(confidence_value),
            tags=[str(tag) for tag in tags],
            source_transform=str(source_transform),
            source_marker_ids=[str(marker_id) for marker_id in source_marker_ids],
            metadata=dict(metadata),
        )

    def _cache_entries_from_artifacts(
        self,
        artifacts: dict[str, str],
        snapshot: _JobSnapshot,
    ) -> list[CacheEntry]:
        cache_entries: list[CacheEntry] = []
        for artifact_kind, artifact_path in artifacts.items():
            cache_entries.append(
                self.cache_store.write_file(
                    artifact_kind,
                    snapshot.dependency_hash,
                    artifact_path,
                    snapshot.transform_version,
                )
            )
        return cache_entries

    def _upsert_cache_entries_locked(
        self,
        project: ProjectDocument,
        cache_entries: list[CacheEntry],
    ) -> None:
        if not cache_entries:
            return
        entry_indexes = {entry.id: index for index, entry in enumerate(project.cache_entries)}
        for entry in cache_entries:
            existing_index = entry_indexes.get(entry.id)
            if existing_index is None:
                entry_indexes[entry.id] = len(project.cache_entries)
                project.cache_entries.append(entry)
            else:
                project.cache_entries[existing_index] = entry

    def _notify_track_changed(self, track_id: str) -> None:
        if self._on_track_changed is None:
            return
        try:
            self._on_track_changed(track_id)
        except Exception:
            pass

    def _cleanup_job(self, job_id: str, future: Future) -> None:
        if not future.done():
            return
        with self._lock:
            if self._futures.get(job_id) is future:
                self._futures.pop(job_id, None)
                self._cancel_events.pop(job_id, None)
