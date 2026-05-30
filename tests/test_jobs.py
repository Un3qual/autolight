import tempfile
import unittest
from pathlib import Path
from threading import Event, Lock, Thread
from unittest.mock import patch

from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import (
    TransformCancelled,
    TransformRegistry,
    TransformResult,
    TransformSpec,
)
from autolight.jobs.queue import LocalJobQueue
from autolight.project.models import Marker, ResultState
from autolight.project.store import (
    add_generated_track,
    create_editable_track_from_markers,
    import_audio_asset,
    new_project,
)


class LocalJobQueueTest(unittest.TestCase):
    def test_successful_job_marks_track_complete_and_adds_markers(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        with tempfile.TemporaryDirectory() as tmp:
            project, track_id = project_with_generated_track(
                Path(tmp), "markers.fixed_interval", {"duration": 1.0, "interval": 0.5}
            )
            queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")
            job_id = queue.submit(project, track_id)

            queue.wait(job_id, timeout=2)

        track = next(track for track in project.tracks if track.id == track_id)
        self.assertEqual(track.result_state, ResultState.COMPLETE)
        self.assertEqual(len([marker for marker in project.markers if marker.track_id == track_id]), 3)

        with tempfile.TemporaryDirectory() as tmp:
            queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")
            job_id = queue.submit(project, track_id)

            queue.wait(job_id, timeout=2)

        self.assertEqual(len([marker for marker in project.markers if marker.track_id == track_id]), 3)

    def test_successful_job_replaces_track_markers_in_single_slice_commit(self):
        def replacement_markers(context, params):
            return TransformResult(markers=[{"timestamp": 2.0, "label": "new"}])

        registry = TransformRegistry()
        registry.register(test_transform("test.marker_replacement", replacement_markers))

        with tempfile.TemporaryDirectory() as tmp:
            project, track_id = project_with_generated_track(Path(tmp), "test.marker_replacement", {})
            project.markers = ObservingMarkerList(
                [Marker(id="marker_old", track_id=track_id, timestamp=1.0)],
                observed_track_id=track_id,
            )
            queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")
            job_id = queue.submit(project, track_id)

            queue.wait(job_id, timeout=2)

        self.assertFalse(project.markers.saw_depleted_track_markers)
        self.assertEqual([marker.timestamp for marker in project.markers if marker.track_id == track_id], [2.0])

    def test_failed_job_keeps_track_and_records_error(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        with tempfile.TemporaryDirectory() as tmp:
            project, track_id = project_with_generated_track(Path(tmp), "markers.fixed_interval", {"interval": 0})
            queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")
            job_id = queue.submit(project, track_id)

            queue.wait(job_id, timeout=2)

        track = next(track for track in project.tracks if track.id == track_id)
        self.assertEqual(track.result_state, ResultState.FAILED)
        self.assertIn("interval", track.error)

    def test_rejects_second_submit_for_running_track(self):
        started = Event()
        release = Event()

        def blocking_transform(context, params):
            started.set()
            release.wait(timeout=1)
            return TransformResult()

        registry = TransformRegistry()
        registry.register(test_transform("test.blocking", blocking_transform))

        with tempfile.TemporaryDirectory() as tmp:
            project, track_id = project_with_generated_track(Path(tmp), "test.blocking", {})
            queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")
            job_id = queue.submit(project, track_id)
            second_job_id = None

            try:
                self.assertTrue(started.wait(timeout=1))
                with self.assertRaises(ValueError):
                    second_job_id = queue.submit(project, track_id)
                running_runs = [
                    run
                    for run in project.job_runs
                    if run.track_id == track_id and run.state == ResultState.RUNNING
                ]
                self.assertEqual(len(running_runs), 1)
            finally:
                release.set()
                queue.wait(job_id, timeout=2)
                if second_job_id is not None:
                    queue.wait(second_job_id, timeout=2)

    def test_finished_job_bookkeeping_is_cleaned_without_wait(self):
        started = Event()
        release = Event()

        def controlled_transform(context, params):
            started.set()
            release.wait(timeout=1)
            return TransformResult()

        registry = TransformRegistry()
        registry.register(test_transform("test.cleanup", controlled_transform))

        with tempfile.TemporaryDirectory() as tmp:
            project, track_id = project_with_generated_track(Path(tmp), "test.cleanup", {})
            queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")
            job_id = queue.submit(project, track_id)
            self.assertTrue(started.wait(timeout=1))
            future = queue._futures[job_id]

            release.set()
            future.result(timeout=2)

            self.assertNotIn(job_id, queue._futures)
            self.assertNotIn(job_id, queue._cancel_events)

    def test_running_job_does_not_commit_after_track_transform_inputs_change(self):
        started = Event()
        release = Event()
        observed_timestamps = []

        def blocking_marker_transform(context, params):
            started.set()
            release.wait(timeout=1)
            observed_timestamps.append(params["timestamp"])
            return TransformResult(markers=[{"timestamp": params["timestamp"], "label": "old"}])

        registry = TransformRegistry()
        registry.register(test_transform("test.stale_commit", blocking_marker_transform))

        with tempfile.TemporaryDirectory() as tmp:
            project, track_id = project_with_generated_track(
                Path(tmp), "test.stale_commit", {"timestamp": 1.0}
            )
            queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")
            job_id = queue.submit(project, track_id)
            self.assertTrue(started.wait(timeout=1))

            track = next(track for track in project.tracks if track.id == track_id)
            track.transform_params["timestamp"] = 2.0
            track.dependency_hash = "changed"
            track.result_state = ResultState.STALE
            track.error = "new transform inputs"
            release.set()
            queue.wait(job_id, timeout=2)

        run = next(run for run in project.job_runs if run.id == job_id)
        self.assertEqual(observed_timestamps, [1.0])
        self.assertEqual(run.state, ResultState.STALE)
        self.assertEqual(track.result_state, ResultState.STALE)
        self.assertEqual(track.error, "new transform inputs")
        self.assertEqual([marker for marker in project.markers if marker.track_id == track_id], [])
        self.assertEqual(track.cache_refs, [])

    def test_failed_stale_job_does_not_mark_changed_track_failed(self):
        started = Event()
        release = Event()

        def failing_transform(context, params):
            started.set()
            release.wait(timeout=1)
            raise ValueError("old job failed")

        registry = TransformRegistry()
        registry.register(test_transform("test.stale_failure", failing_transform))

        with tempfile.TemporaryDirectory() as tmp:
            project, track_id = project_with_generated_track(
                Path(tmp), "test.stale_failure", {"timestamp": 1.0}
            )
            queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")
            job_id = queue.submit(project, track_id)
            self.assertTrue(started.wait(timeout=1))

            track = next(track for track in project.tracks if track.id == track_id)
            track.transform_params["timestamp"] = 2.0
            track.dependency_hash = "changed"
            track.result_state = ResultState.STALE
            track.error = "new transform inputs"
            release.set()
            queue.wait(job_id, timeout=2)

        run = next(run for run in project.job_runs if run.id == job_id)
        self.assertEqual(run.state, ResultState.STALE)
        self.assertIn("track changed", run.error)
        self.assertNotEqual(track.result_state, ResultState.FAILED)
        self.assertEqual(track.result_state, ResultState.STALE)
        self.assertEqual(track.error, "new transform inputs")
        self.assertEqual([marker for marker in project.markers if marker.track_id == track_id], [])
        self.assertEqual(track.cache_refs, [])

    def test_cancelled_job_marks_track_and_run_cancelled(self):
        started = Event()
        release = Event()

        def cancellable_transform(context, params):
            started.set()
            release.wait(timeout=1)
            if context.cancel_requested():
                raise TransformCancelled("cancelled")
            return TransformResult(markers=[{"timestamp": 0.0}])

        registry = TransformRegistry()
        registry.register(test_transform("test.cancellable", cancellable_transform))

        with tempfile.TemporaryDirectory() as tmp:
            project, track_id = project_with_generated_track(Path(tmp), "test.cancellable", {})
            queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")
            job_id = queue.submit(project, track_id)
            self.assertTrue(started.wait(timeout=1))
            queue.cancel(job_id)
            release.set()
            queue.wait(job_id, timeout=2)

        track = next(track for track in project.tracks if track.id == track_id)
        run = next(run for run in project.job_runs if run.id == job_id)
        self.assertEqual(track.result_state, ResultState.CANCELLED)
        self.assertEqual(run.state, ResultState.CANCELLED)
        self.assertEqual([marker for marker in project.markers if marker.track_id == track_id], [])

    def test_cancel_race_cannot_commit_complete_after_cancel_starts(self):
        started = Event()
        release = Event()
        cancel_events = []

        def delayed_event_factory():
            event = CommitWindowCancelEvent()
            cancel_events.append(event)
            return event

        def cancellable_transform(context, params):
            started.set()
            release.wait(timeout=1)
            return TransformResult(markers=[{"timestamp": 0.0}])

        registry = TransformRegistry()
        registry.register(test_transform("test.cancel_commit_race", cancellable_transform))

        with tempfile.TemporaryDirectory() as tmp:
            project, track_id = project_with_generated_track(Path(tmp), "test.cancel_commit_race", {})
            with patch("autolight.jobs.queue.Event", side_effect=delayed_event_factory):
                queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")
                job_id = queue.submit(project, track_id)
                self.assertTrue(started.wait(timeout=1))
                cancel_event = cancel_events[0]

                cancel_thread = Thread(target=queue.cancel, args=(job_id,))
                cancel_thread.start()
                self.assertTrue(cancel_event.set_started.wait(timeout=1))
                release.set()
                cancel_thread.join(timeout=1)
                self.assertFalse(cancel_thread.is_alive())
                queue.wait(job_id, timeout=2)

        track = next(track for track in project.tracks if track.id == track_id)
        run = next(run for run in project.job_runs if run.id == job_id)
        self.assertEqual(track.result_state, ResultState.CANCELLED)
        self.assertEqual(run.state, ResultState.CANCELLED)
        self.assertEqual([marker for marker in project.markers if marker.track_id == track_id], [])

    def test_lookup_failure_marks_track_failed_and_records_completion(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        with tempfile.TemporaryDirectory() as tmp:
            project, track_id = project_with_generated_track(
                Path(tmp),
                "markers.fixed_interval",
                {"duration": 1.0, "interval": 0.5},
                transform_version="2",
            )
            queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")
            job_id = queue.submit(project, track_id)

            queue.wait(job_id, timeout=2)

        track = next(track for track in project.tracks if track.id == track_id)
        run = next(run for run in project.job_runs if run.id == job_id)
        self.assertEqual(track.result_state, ResultState.FAILED)
        self.assertEqual(run.state, ResultState.FAILED)
        self.assertIn("version mismatch", track.error)
        self.assertEqual(run.error, track.error)
        self.assertNotEqual(run.completed_at, "")

    def test_job_creates_artifact_dir_before_running_transform(self):
        def assert_artifact_dir_exists(context, params):
            if not context.artifact_dir.is_dir():
                raise AssertionError(f"missing artifact dir: {context.artifact_dir}")
            (context.artifact_dir / "output.json").write_text("{}", encoding="utf-8")
            return TransformResult(artifacts={"output": str(context.artifact_dir / "output.json")})

        registry = TransformRegistry()
        registry.register(
            TransformSpec(
                id="test.artifact_dir",
                version="1",
                name="Artifact Dir Test",
                input_schema="audio.v1",
                output_schema="artifact.test.v1",
                estimated_cost="light",
                run=assert_artifact_dir_exists,
            )
        )

        with tempfile.TemporaryDirectory() as tmp:
            project, track_id = project_with_generated_track(Path(tmp), "test.artifact_dir", {})
            queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")
            job_id = queue.submit(project, track_id)

            queue.wait(job_id, timeout=2)

        track = next(track for track in project.tracks if track.id == track_id)
        run = next(run for run in project.job_runs if run.id == job_id)
        self.assertEqual(track.result_state, ResultState.COMPLETE)
        self.assertEqual(run.state, ResultState.COMPLETE)
        self.assertNotEqual(track.cache_refs, [])
        self.assertEqual(track.cache_refs, run.produced_cache_refs)
        self.assertEqual(track.cache_refs, [entry.id for entry in project.cache_entries])
        self.assertFalse(Path(track.cache_refs[0]).is_absolute())

    def test_job_removes_artifact_work_dir_after_completion(self):
        artifact_dirs = []

        def writes_artifact(context, params):
            artifact_dirs.append(context.artifact_dir)
            artifact = context.artifact_dir / "output.json"
            artifact.write_text("{}", encoding="utf-8")
            return TransformResult(artifacts={"output": str(artifact)})

        registry = TransformRegistry()
        registry.register(test_transform("test.work_dir_cleanup", writes_artifact))

        with tempfile.TemporaryDirectory() as tmp:
            project, track_id = project_with_generated_track(Path(tmp), "test.work_dir_cleanup", {})
            queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")
            job_id = queue.submit(project, track_id)

            queue.wait(job_id, timeout=2)

            self.assertEqual(len(artifact_dirs), 1)
            self.assertFalse(artifact_dirs[0].exists())

    def test_successful_job_marks_dependent_tracks_stale(self):
        def marker_transform(context, params):
            return TransformResult(markers=[{"timestamp": params["timestamp"]}])

        registry = TransformRegistry()
        registry.register(test_transform("test.upstream", marker_transform))

        with tempfile.TemporaryDirectory() as tmp:
            project, upstream_id = project_with_generated_track(
                Path(tmp), "test.upstream", {"timestamp": 1.0}
            )
            downstream = add_generated_track(
                project,
                upstream_id,
                "Pitch",
                "test.downstream",
                {},
                "1",
                "markers.v1",
                "dep_downstream",
            )
            downstream.result_state = ResultState.COMPLETE
            editable = create_editable_track_from_markers(project, upstream_id, "Edited Beats", [])
            editable.result_state = ResultState.COMPLETE
            queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")
            job_id = queue.submit(project, upstream_id)

            queue.wait(job_id, timeout=2)

        upstream = next(track for track in project.tracks if track.id == upstream_id)
        self.assertEqual(upstream.result_state, ResultState.COMPLETE)
        self.assertEqual(downstream.result_state, ResultState.STALE)
        self.assertEqual(editable.result_state, ResultState.STALE)

    def test_running_dependent_job_does_not_commit_after_upstream_stales_it(self):
        downstream_started = Event()
        downstream_release = Event()

        def upstream_transform(context, params):
            return TransformResult(markers=[{"timestamp": 1.0, "label": "upstream"}])

        def downstream_transform(context, params):
            downstream_started.set()
            downstream_release.wait(timeout=1)
            return TransformResult(markers=[{"timestamp": 9.0, "label": "stale downstream"}])

        registry = TransformRegistry()
        registry.register(test_transform("test.upstream_stales_running_dependent", upstream_transform))
        registry.register(test_transform("test.running_dependent", downstream_transform))

        with tempfile.TemporaryDirectory() as tmp:
            project, upstream_id = project_with_generated_track(
                Path(tmp), "test.upstream_stales_running_dependent", {}
            )
            downstream = add_generated_track(
                project,
                upstream_id,
                "Dependent",
                "test.running_dependent",
                {},
                "1",
                "markers.v1",
                "dep_downstream",
            )
            downstream.result_state = ResultState.COMPLETE
            queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")

            downstream_job_id = queue.submit(project, downstream.id)
            self.assertTrue(downstream_started.wait(timeout=1))
            upstream_job_id = queue.submit(project, upstream_id)
            queue.wait(upstream_job_id, timeout=2)
            self.assertEqual(downstream.result_state, ResultState.STALE)

            downstream_release.set()
            queue.wait(downstream_job_id, timeout=2)

        downstream_run = next(run for run in project.job_runs if run.id == downstream_job_id)
        self.assertEqual(downstream.result_state, ResultState.STALE)
        self.assertEqual(downstream_run.state, ResultState.STALE)
        self.assertEqual([marker for marker in project.markers if marker.track_id == downstream.id], [])

    def test_artifact_job_records_cache_entry_and_can_mark_missing_artifact_stale(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        with tempfile.TemporaryDirectory() as tmp:
            project, track_id = project_with_generated_track(
                Path(tmp),
                "stems.vocals_stand_in",
                {"label": "vocals"},
            )
            queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")
            job_id = queue.submit(project, track_id)

            queue.wait(job_id, timeout=2)

            track = next(track for track in project.tracks if track.id == track_id)
            self.assertEqual(track.result_state, ResultState.COMPLETE)
            self.assertEqual(len(project.cache_entries), 1)
            self.assertEqual(track.cache_refs, [project.cache_entries[0].id])
            self.assertEqual(project.cache_entries[0].artifact_kind, "stem")
            cached_path = queue.cache_store.artifact_path(project.cache_entries[0])
            self.assertTrue(cached_path.is_file())
            self.assertEqual(cached_path.read_text(encoding="utf-8"), '{"samples": [], "stem": "vocals"}')

            cached_path.unlink()
            invalid_refs = queue.refresh_cache_validity(project)

            self.assertEqual(invalid_refs, [project.cache_entries[0].id])
            self.assertEqual(track.result_state, ResultState.STALE)
            self.assertIn("cache artifact", track.error)

    def test_artifact_job_streams_artifact_into_cache(self):
        def writes_artifact(context, params):
            artifact = context.artifact_dir / "large-stem.wav"
            artifact.write_bytes(b"large artifact placeholder")
            return TransformResult(artifacts={"stem": str(artifact)})

        registry = TransformRegistry()
        registry.register(test_transform("test.streaming_artifact", writes_artifact))

        with tempfile.TemporaryDirectory() as tmp:
            project, track_id = project_with_generated_track(Path(tmp), "test.streaming_artifact", {})
            queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")
            with patch.object(Path, "read_bytes", side_effect=AssertionError("whole-file read")):
                job_id = queue.submit(project, track_id)
                queue.wait(job_id, timeout=2)

        track = next(track for track in project.tracks if track.id == track_id)
        self.assertEqual(track.result_state, ResultState.COMPLETE)
        self.assertEqual(len(project.cache_entries), 1)
        self.assertEqual(track.cache_refs, [project.cache_entries[0].id])

    def test_track_change_callback_fires_for_running_and_complete_states(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        with tempfile.TemporaryDirectory() as tmp:
            project, track_id = project_with_generated_track(
                Path(tmp), "markers.fixed_interval", {"duration": 1.0, "interval": 0.5}
            )
            changed_track_ids = []
            queue = LocalJobQueue(
                registry,
                artifact_root=Path(tmp) / "artifacts",
                on_track_changed=changed_track_ids.append,
            )

            job_id = queue.submit(project, track_id)
            queue.wait(job_id, timeout=2)

        self.assertEqual(changed_track_ids[0], track_id)
        self.assertEqual(changed_track_ids[-1], track_id)
        self.assertGreaterEqual(len(changed_track_ids), 2)

    def test_malformed_marker_output_leaves_no_partial_markers(self):
        def malformed_markers(context, params):
            return TransformResult(
                markers=[
                    {"timestamp": 0.0, "label": "valid"},
                    {"label": "missing timestamp"},
                ]
            )

        registry = TransformRegistry()
        registry.register(test_transform("test.malformed_markers", malformed_markers))

        with tempfile.TemporaryDirectory() as tmp:
            project, track_id = project_with_generated_track(Path(tmp), "test.malformed_markers", {})
            queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")
            job_id = queue.submit(project, track_id)

            queue.wait(job_id, timeout=2)

        track = next(track for track in project.tracks if track.id == track_id)
        self.assertEqual(track.result_state, ResultState.FAILED)
        self.assertEqual([marker for marker in project.markers if marker.track_id == track_id], [])

    def test_rich_marker_data_is_preserved(self):
        def rich_markers(context, params):
            return TransformResult(
                markers=[
                    {
                        "timestamp": 1.25,
                        "duration": 0.5,
                        "label": "Chorus",
                        "category": "section",
                        "confidence": 0.75,
                        "tags": ["hook", "repeat"],
                        "source_marker_ids": ["marker_a", "marker_b"],
                        "source_transform": "source.transform",
                        "metadata": {"energy": "high"},
                    }
                ]
            )

        registry = TransformRegistry()
        registry.register(test_transform("test.rich_markers", rich_markers))

        with tempfile.TemporaryDirectory() as tmp:
            project, track_id = project_with_generated_track(Path(tmp), "test.rich_markers", {})
            queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")
            job_id = queue.submit(project, track_id)

            queue.wait(job_id, timeout=2)

        marker = next(marker for marker in project.markers if marker.track_id == track_id)
        self.assertEqual(marker.timestamp, 1.25)
        self.assertEqual(marker.duration, 0.5)
        self.assertEqual(marker.label, "Chorus")
        self.assertEqual(marker.category, "section")
        self.assertEqual(marker.confidence, 0.75)
        self.assertEqual(marker.tags, ["hook", "repeat"])
        self.assertEqual(marker.source_marker_ids, ["marker_a", "marker_b"])
        self.assertEqual(marker.source_transform, "source.transform")
        self.assertEqual(marker.metadata, {"energy": "high"})


def project_with_generated_track(
    tmp: Path, transform_id: str, params: dict, transform_version: str = "1"
):
    audio_path = tmp / "song.wav"
    audio_path.write_bytes(b"audio")
    project = new_project("Demo")
    source = import_audio_asset(project, audio_path)
    generated = add_generated_track(
        project,
        parent_track_id=source.id,
        name="Generated",
        transform_id=transform_id,
        transform_params=params,
        transform_version=transform_version,
        output_schema="markers.v1",
        dependency_hash="dep",
    )
    return project, generated.id


def test_transform(transform_id: str, run):
    return TransformSpec(
        id=transform_id,
        version="1",
        name="Test Transform",
        input_schema="audio.v1",
        output_schema="markers.v1",
        estimated_cost="light",
        run=run,
    )


class ObservingMarkerList(list):
    def __init__(self, values, observed_track_id):
        super().__init__(values)
        self.observed_track_id = observed_track_id
        self.saw_depleted_track_markers = False

    def __setitem__(self, key, value):
        super().__setitem__(key, value)
        if isinstance(key, slice) and not any(
            marker.track_id == self.observed_track_id for marker in self
        ):
            self.saw_depleted_track_markers = True


class CommitWindowCancelEvent:
    def __init__(self):
        self._event = Event()
        self._lock = Lock()
        self._pending_checks = 0
        self.set_started = Event()
        self.commit_window_checked = Event()

    def set(self):
        self.set_started.set()
        self.commit_window_checked.wait(timeout=0.2)
        self._event.set()

    def is_set(self):
        if self.set_started.is_set() and not self._event.is_set():
            with self._lock:
                self._pending_checks += 1
                if self._pending_checks >= 2:
                    self.commit_window_checked.set()
        return self._event.is_set()


if __name__ == "__main__":
    unittest.main()
