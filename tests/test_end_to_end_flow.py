import tempfile
import unittest
from pathlib import Path

from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformRegistry
from autolight.jobs.queue import LocalJobQueue
from autolight.project.models import ResultState
from autolight.project.store import (
    ProjectStore,
    add_generated_track,
    create_editable_track_from_markers,
    import_audio_asset,
    new_project,
)


class EndToEndFlowTest(unittest.TestCase):
    def test_import_run_derive_save_and_load(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            audio_path.write_bytes(b"audio")
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
            job_id = queue.submit(project, generated.id)
            queue.wait(job_id, timeout=2)

            source_marker_ids = [marker.id for marker in project.markers if marker.track_id == generated.id]
            editable = create_editable_track_from_markers(project, generated.id, "Editable Cues", source_marker_ids)
            ProjectStore.save(project, project_path)
            loaded = ProjectStore.load(project_path)

        loaded_generated = next(track for track in loaded.tracks if track.id == generated.id)
        loaded_editable = next(track for track in loaded.tracks if track.id == editable.id)
        self.assertEqual(loaded_generated.result_state, ResultState.COMPLETE)
        self.assertEqual(loaded_editable.result_state, ResultState.COMPLETE)
        self.assertEqual(len([marker for marker in loaded.markers if marker.track_id == generated.id]), 3)
        self.assertEqual(loaded_editable.provenance["source_marker_ids"], source_marker_ids)


if __name__ == "__main__":
    unittest.main()
