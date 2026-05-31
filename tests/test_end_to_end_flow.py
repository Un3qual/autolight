import tempfile
import unittest
import wave
from pathlib import Path

from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformRegistry
from autolight.jobs.queue import LocalJobQueue
from autolight.project.models import ResultState, TrackType
from autolight.project.store import (
    ProjectStore,
    add_generated_track,
    create_editable_track_from_markers,
    import_audio_asset,
    new_project,
)


def write_wav(path: Path) -> None:
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(8000)
        handle.writeframes(b"\0\0" * 8000)


class EndToEndFlowTest(unittest.TestCase):
    def test_import_run_derive_save_and_load(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            write_wav(audio_path)
            project_path = root / "show.autolight"
            project = new_project("Demo")
            source = import_audio_asset(project, audio_path)
            generated = add_generated_track(
                project,
                parent_track_id=source.id,
                name="Beat Markers",
                transform_id="markers.fixed_interval",
                transform_params={"duration": 1.0, "interval": 0.5},
                transform_version="1",
                output_schema="markers.v1",
                dependency_hash="dep",
            )
            queue = LocalJobQueue(registry, artifact_root=root / "artifacts")
            self.addCleanup(queue.shutdown)
            job_id = queue.submit(project, generated.id)
            queue.wait(job_id, timeout=2)
            job_run = next(run for run in project.job_runs if run.id == job_id)
            self.assertEqual(generated.result_state, ResultState.COMPLETE)
            self.assertEqual(job_run.state, ResultState.COMPLETE)

            source_marker_ids = [marker.id for marker in project.markers if marker.track_id == generated.id]
            editable = create_editable_track_from_markers(project, generated.id, "Editable Cues", source_marker_ids)
            ProjectStore.save(project, project_path)
            loaded = ProjectStore.load(project_path)

        loaded_source = next(track for track in loaded.tracks if track.id == source.id)
        loaded_generated = next(track for track in loaded.tracks if track.id == generated.id)
        loaded_editable = next(track for track in loaded.tracks if track.id == editable.id)
        loaded_job_run = next(run for run in loaded.job_runs if run.id == job_id)
        self.assertEqual(len(loaded.audio_assets), 1)
        self.assertEqual(len(loaded.tracks), 3)
        self.assertEqual(len(loaded.job_runs), 1)
        self.assertEqual(loaded_source.type, TrackType.SOURCE)
        self.assertEqual(loaded_source.result_state, ResultState.COMPLETE)
        self.assertEqual(loaded_generated.input_track_ids, [source.id])
        self.assertEqual(loaded_editable.input_track_ids, [generated.id])
        self.assertEqual(loaded_generated.result_state, ResultState.COMPLETE)
        self.assertEqual(loaded_editable.result_state, ResultState.COMPLETE)
        self.assertEqual(len([marker for marker in loaded.markers if marker.track_id == generated.id]), 3)
        self.assertEqual(loaded_editable.provenance["source_marker_ids"], source_marker_ids)
        self.assertEqual(loaded_job_run.state, ResultState.COMPLETE)
        self.assertEqual(loaded_job_run.track_id, generated.id)
        self.assertEqual(loaded_job_run.transform_id, "markers.fixed_interval")
        self.assertEqual(loaded_job_run.progress, 1.0)
        self.assertNotEqual(loaded_job_run.started_at, "")
        self.assertNotEqual(loaded_job_run.completed_at, "")


if __name__ == "__main__":
    unittest.main()
