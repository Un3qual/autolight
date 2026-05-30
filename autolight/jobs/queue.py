from __future__ import annotations

from concurrent.futures import Future, ThreadPoolExecutor
from datetime import datetime, timezone
from pathlib import Path
from threading import Event, Lock

from autolight.analysis.registry import TransformCancelled, TransformContext, TransformRegistry
from autolight.project.models import JobRun, Marker, ProjectDocument, ResultState, Track
from autolight.project.store import find_track, new_id


class LocalJobQueue:
    def __init__(self, registry: TransformRegistry, artifact_root: Path):
        self.registry = registry
        self.artifact_root = artifact_root
        self.artifact_root.mkdir(parents=True, exist_ok=True)
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
            run = JobRun(
                id=job_id,
                track_id=track_id,
                transform_id=track.transform_id,
                parameters_hash=track.dependency_hash,
                state=ResultState.RUNNING,
                started_at=datetime.now(timezone.utc).isoformat(),
            )
            project.job_runs.append(run)
            track.result_state = ResultState.RUNNING
            track.error = ""
            self._active_job_by_track[track_id] = job_id

            future = self._executor.submit(self._run, project, track, run, cancel_event)
            self._futures[job_id] = future
            self._cancel_events[job_id] = cancel_event
            return job_id

    def cancel(self, job_id: str) -> None:
        with self._lock:
            event = self._cancel_events.get(job_id)
        if event is not None:
            event.set()

    def wait(self, job_id: str, timeout: float | None = None) -> None:
        with self._lock:
            future = self._futures[job_id]
        try:
            future.result(timeout=timeout)
        finally:
            if future.done():
                with self._lock:
                    self._futures.pop(job_id, None)
                    self._cancel_events.pop(job_id, None)

    def shutdown(self) -> None:
        self._executor.shutdown(wait=True)

    def _run(self, project: ProjectDocument, track: Track, run: JobRun, cancel_event: Event) -> None:
        artifact_dir = self.artifact_root / run.id

        def progress(value: float) -> None:
            with self._lock:
                run.progress = max(0.0, min(1.0, value))

        try:
            transform = self.registry.get(track.transform_id, version=track.transform_version)
            artifact_dir.mkdir(parents=True, exist_ok=True)
            context = TransformContext(
                artifact_dir=artifact_dir,
                cancel_requested=cancel_event.is_set,
                progress=progress,
            )
            result = transform.run(context, track.transform_params)
            if cancel_event.is_set():
                self._mark_finished(track, run, ResultState.CANCELLED)
                return
            markers = [self._marker_from_result(track, item) for item in result.markers]
            cache_refs = list(result.artifacts.values())
            with self._lock:
                if cancel_event.is_set():
                    self._mark_finished_locked(track, run, ResultState.CANCELLED)
                    return
                run.state = ResultState.COMPLETE
                run.progress = 1.0
                run.produced_cache_refs = cache_refs
                if self._active_job_by_track.get(track.id) == run.id:
                    project.markers[:] = [
                        marker for marker in project.markers if marker.track_id != track.id
                    ]
                    project.markers.extend(markers)
                    track.cache_refs = cache_refs
                    track.result_state = ResultState.COMPLETE
                    track.error = ""
        except TransformCancelled:
            self._mark_finished(track, run, ResultState.CANCELLED)
        except Exception as exc:
            self._mark_finished(track, run, ResultState.FAILED, error=str(exc))
        finally:
            with self._lock:
                run.completed_at = datetime.now(timezone.utc).isoformat()
                if self._active_job_by_track.get(track.id) == run.id:
                    self._active_job_by_track.pop(track.id, None)

    def _mark_finished(
        self, track: Track, run: JobRun, state: ResultState, error: str = ""
    ) -> None:
        with self._lock:
            self._mark_finished_locked(track, run, state, error=error)

    def _mark_finished_locked(
        self, track: Track, run: JobRun, state: ResultState, error: str = ""
    ) -> None:
        run.state = state
        run.error = error
        if self._active_job_by_track.get(track.id) == run.id:
            track.result_state = state
            track.error = error

    def _marker_from_result(self, track: Track, item: dict) -> Marker:
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
        source_transform = item.get("source_transform", track.transform_id)

        if tags is None:
            tags = []
        if source_marker_ids is None:
            source_marker_ids = []
        if metadata is None:
            metadata = {}
        if source_transform is None:
            source_transform = track.transform_id
        if not isinstance(tags, list):
            raise ValueError("marker tags must be a list")
        if not isinstance(source_marker_ids, list):
            raise ValueError("marker source_marker_ids must be a list")
        if not isinstance(metadata, dict):
            raise ValueError("marker metadata must be a dict")

        return Marker(
            id=new_id("marker"),
            track_id=track.id,
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
