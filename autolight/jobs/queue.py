from __future__ import annotations

from concurrent.futures import Future, ThreadPoolExecutor
from datetime import datetime, timezone
from pathlib import Path
from threading import Event

from autolight.analysis.registry import TransformCancelled, TransformContext, TransformRegistry
from autolight.project.models import JobRun, Marker, ProjectDocument, ResultState, Track
from autolight.project.store import find_track, new_id


class LocalJobQueue:
    def __init__(self, registry: TransformRegistry, artifact_root: Path):
        self.registry = registry
        self.artifact_root = artifact_root
        self.artifact_root.mkdir(parents=True, exist_ok=True)
        self._executor = ThreadPoolExecutor(max_workers=2)
        self._futures: dict[str, Future] = {}
        self._cancel_events: dict[str, Event] = {}

    def submit(self, project: ProjectDocument, track_id: str) -> str:
        track = find_track(project, track_id)
        if track is None:
            raise ValueError(f"track not found: {track_id}")
        if not track.transform_id:
            raise ValueError("track has no transform")

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

        future = self._executor.submit(self._run, project, track, run, cancel_event)
        self._futures[job_id] = future
        self._cancel_events[job_id] = cancel_event
        return job_id

    def cancel(self, job_id: str) -> None:
        event = self._cancel_events.get(job_id)
        if event is not None:
            event.set()

    def wait(self, job_id: str, timeout: float | None = None) -> None:
        self._futures[job_id].result(timeout=timeout)

    def shutdown(self) -> None:
        self._executor.shutdown(wait=True)

    def _run(self, project: ProjectDocument, track: Track, run: JobRun, cancel_event: Event) -> None:
        transform = self.registry.get(track.transform_id, version=track.transform_version)
        artifact_dir = self.artifact_root / run.id

        def progress(value: float) -> None:
            run.progress = max(0.0, min(1.0, value))

        context = TransformContext(artifact_dir=artifact_dir, cancel_requested=cancel_event.is_set, progress=progress)
        try:
            result = transform.run(context, track.transform_params)
            if cancel_event.is_set():
                track.result_state = ResultState.CANCELLED
                run.state = ResultState.CANCELLED
                return
            for item in result.markers:
                project.markers.append(
                    Marker(
                        id=new_id("marker"),
                        track_id=track.id,
                        timestamp=float(item["timestamp"]),
                        label=str(item.get("label", "")),
                        category=str(item.get("category", "")),
                        confidence=item.get("confidence"),
                        source_transform=track.transform_id,
                        metadata=dict(item.get("metadata", {})),
                    )
                )
            track.cache_refs = list(result.artifacts.values())
            track.result_state = ResultState.COMPLETE
            run.state = ResultState.COMPLETE
            run.progress = 1.0
            run.produced_cache_refs = list(result.artifacts.values())
        except TransformCancelled:
            track.result_state = ResultState.CANCELLED
            run.state = ResultState.CANCELLED
        except Exception as exc:
            track.result_state = ResultState.FAILED
            run.state = ResultState.FAILED
            track.error = str(exc)
            run.error = str(exc)
        finally:
            run.completed_at = datetime.now(timezone.utc).isoformat()
