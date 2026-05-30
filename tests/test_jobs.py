import tempfile
import time
import unittest
from pathlib import Path

from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformRegistry, TransformResult, TransformSpec
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

    def test_cancelled_job_does_not_mark_track_complete(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        with tempfile.TemporaryDirectory() as tmp:
            project, track_id = project_with_generated_track(
                Path(tmp), "stems.vocals_stand_in", {"label": "vocals"}
            )
            queue = LocalJobQueue(registry, artifact_root=Path(tmp) / "artifacts")
            job_id = queue.submit(project, track_id)
            time.sleep(0.005)
            queue.cancel(job_id)
            queue.wait(job_id, timeout=2)

        track = next(track for track in project.tracks if track.id == track_id)
        self.assertIn(track.result_state, {ResultState.CANCELLED, ResultState.FAILED})
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


def project_with_generated_track(tmp: Path, transform_id: str, params: dict, transform_version: str = "1"):
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


if __name__ == "__main__":
    unittest.main()
