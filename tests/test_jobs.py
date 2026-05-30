import tempfile
import time
import unittest
from pathlib import Path

from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformRegistry
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


def project_with_generated_track(tmp: Path, transform_id: str, params: dict):
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
        transform_version="1",
        output_schema="markers.v1",
        dependency_hash="dep",
    )
    return project, generated.id


if __name__ == "__main__":
    unittest.main()
