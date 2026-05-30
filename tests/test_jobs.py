import tempfile
import unittest
from pathlib import Path
from threading import Event

from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import (
    TransformCancelled,
    TransformRegistry,
    TransformResult,
    TransformSpec,
)
from autolight.jobs.queue import LocalJobQueue
from autolight.project.models import ResultState
from autolight.project.store import add_generated_track, import_audio_asset, new_project


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


if __name__ == "__main__":
    unittest.main()
