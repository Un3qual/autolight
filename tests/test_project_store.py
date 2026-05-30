import json
import tempfile
import unittest
from pathlib import Path

from autolight.project.models import ProjectDocument, ResultState, Track, TrackType
from autolight.project.store import (
    ProjectStore,
    add_generated_track,
    create_editable_track_from_markers,
    import_audio_asset,
    mark_dependents_stale,
    new_project,
    validate_graph,
)


class ProjectStoreTest(unittest.TestCase):
    def test_save_and_load_project_round_trip(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            audio_path.write_bytes(b"fake audio bytes")
            project_path = Path(tmp) / "show.autolight"

            project = new_project("Demo")
            source_track = import_audio_asset(project, audio_path)
            add_generated_track(
                project,
                parent_track_id=source_track.id,
                name="Beat Markers",
                transform_id="markers.beats",
                transform_params={"interval": 0.5},
                transform_version="1",
                output_schema="markers.v1",
                dependency_hash="hash_1",
            )

            ProjectStore.save(project, project_path)
            loaded = ProjectStore.load(project_path)

            self.assertEqual(loaded, project)

    def test_load_rejects_persisted_project_with_invalid_graph(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            audio_path.write_bytes(b"fake audio bytes")
            project_path = Path(tmp) / "show.autolight"

            project = new_project("Demo")
            source_track = import_audio_asset(project, audio_path)
            add_generated_track(
                project,
                parent_track_id=source_track.id,
                name="Beat Markers",
                transform_id="markers.beats",
                transform_params={},
                transform_version="1",
                output_schema="markers.v1",
                dependency_hash="hash_1",
            )
            ProjectStore.save(project, project_path)
            raw = json.loads(project_path.read_text(encoding="utf-8"))
            raw["tracks"][1]["input_track_ids"] = ["missing_track"]
            project_path.write_text(json.dumps(raw), encoding="utf-8")

            with self.assertRaisesRegex(ValueError, "missing input track"):
                ProjectStore.load(project_path)

    def test_load_rejects_unsupported_schema_version(self):
        with tempfile.TemporaryDirectory() as tmp:
            project_path = Path(tmp) / "show.autolight"
            project = new_project("Demo")
            ProjectStore.save(project, project_path)
            raw = json.loads(project_path.read_text(encoding="utf-8"))
            raw["schema_version"] = 999
            project_path.write_text(json.dumps(raw), encoding="utf-8")

            with self.assertRaisesRegex(ValueError, "unsupported schema version"):
                ProjectStore.load(project_path)

    def test_single_parent_validation_rejects_generated_track_with_two_inputs(self):
        project = new_project("Demo")
        project.tracks.append(
            Track(
                id="track_invalid",
                type=TrackType.GENERATED,
                name="Invalid",
                input_track_ids=["a", "b"],
            )
        )

        with self.assertRaisesRegex(ValueError, "exactly one input"):
            validate_graph(project)

    def test_graph_validation_rejects_direct_cycle(self):
        project = ProjectDocument(
            id="project_1",
            name="Demo",
            tracks=[
                Track(
                    id="track_loop",
                    type=TrackType.GENERATED,
                    name="Loop",
                    input_track_ids=["track_loop"],
                )
            ],
        )

        with self.assertRaisesRegex(ValueError, "cycle"):
            validate_graph(project)

    def test_graph_validation_rejects_indirect_cycle(self):
        project = ProjectDocument(
            id="project_1",
            name="Demo",
            tracks=[
                Track(
                    id="track_a",
                    type=TrackType.GENERATED,
                    name="A",
                    input_track_ids=["track_b"],
                ),
                Track(
                    id="track_b",
                    type=TrackType.GENERATED,
                    name="B",
                    input_track_ids=["track_a"],
                ),
            ],
        )

        with self.assertRaisesRegex(ValueError, "cycle"):
            validate_graph(project)

    def test_generated_track_helper_appends_after_parent_validation(self):
        project = new_project("Demo")
        source = import_audio_asset_from_bytes(project, b"audio")

        generated = add_generated_track(project, source.id, "Beats", "markers.beats", {}, "1", "markers.v1", "h1")

        self.assertIs(project.tracks[-1], generated)

    def test_import_audio_asset_rejects_directory(self):
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(IsADirectoryError, "not a file"):
                import_audio_asset(new_project("Demo"), Path(tmp))

    def test_stale_propagation_marks_generated_descendants_only(self):
        project = new_project("Demo")
        source = import_audio_asset_from_bytes(project, b"audio")
        beat = add_generated_track(project, source.id, "Beats", "markers.beats", {}, "1", "markers.v1", "h1")
        edit = create_editable_track_from_markers(project, beat.id, "Edited Beats", [])
        pitch = add_generated_track(project, beat.id, "Pitch", "pitch.basic", {}, "1", "markers.v1", "h2")
        beat.result_state = ResultState.COMPLETE
        edit.result_state = ResultState.COMPLETE
        pitch.result_state = ResultState.COMPLETE

        mark_dependents_stale(project, beat.id)

        self.assertEqual(edit.result_state, ResultState.COMPLETE)
        self.assertEqual(pitch.result_state, ResultState.STALE)


def import_audio_asset_from_bytes(project, payload: bytes):
    with tempfile.TemporaryDirectory() as tmp:
        audio_path = Path(tmp) / "song.wav"
        audio_path.write_bytes(payload)
        return import_audio_asset(project, audio_path)


if __name__ == "__main__":
    unittest.main()
