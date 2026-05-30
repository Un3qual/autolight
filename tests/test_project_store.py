import json
import tempfile
import unittest
from pathlib import Path

from autolight.project.models import (
    AudioAsset,
    CacheEntry,
    JobRun,
    Marker,
    ProjectDocument,
    ResultState,
    Track,
    TrackType,
)
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

    def test_editable_track_clones_selected_source_markers(self):
        project = new_project("Demo")
        source = import_audio_asset_from_bytes(project, b"audio")
        beat = add_generated_track(project, source.id, "Beats", "markers.beats", {}, "1", "markers.v1", "h1")
        project.markers.append(
            Marker(
                id="marker_1",
                track_id=beat.id,
                timestamp=1.25,
                duration=0.5,
                label="Beat",
                category="timing",
                confidence=0.8,
                tags=["strong"],
                source_transform="markers.beats",
                metadata={"energy": "high"},
            )
        )

        edit = create_editable_track_from_markers(project, beat.id, "Edited Beats", ["marker_1"])

        copied = [marker for marker in project.markers if marker.track_id == edit.id]
        self.assertEqual(len(copied), 1)
        self.assertEqual(copied[0].timestamp, 1.25)
        self.assertEqual(copied[0].duration, 0.5)
        self.assertEqual(copied[0].label, "Beat")
        self.assertEqual(copied[0].category, "timing")
        self.assertEqual(copied[0].confidence, 0.8)
        self.assertEqual(copied[0].tags, ["strong"])
        self.assertEqual(copied[0].source_transform, "markers.beats")
        self.assertEqual(copied[0].source_marker_ids, ["marker_1"])
        self.assertEqual(copied[0].metadata, {"energy": "high"})

    def test_editable_track_rejects_missing_or_foreign_source_markers(self):
        project = new_project("Demo")
        source = import_audio_asset_from_bytes(project, b"audio")
        beat = add_generated_track(project, source.id, "Beats", "markers.beats", {}, "1", "markers.v1", "h1")
        pitch = add_generated_track(project, beat.id, "Pitch", "pitch.basic", {}, "1", "markers.v1", "h2")
        project.markers.append(Marker(id="marker_pitch", track_id=pitch.id, timestamp=2.0))

        with self.assertRaisesRegex(ValueError, "source marker not found"):
            create_editable_track_from_markers(project, beat.id, "Edited Beats", ["missing"])

        with self.assertRaisesRegex(ValueError, "source marker not found"):
            create_editable_track_from_markers(project, beat.id, "Edited Beats", ["marker_pitch"])

        self.assertEqual([track for track in project.tracks if track.type == TrackType.EDITABLE], [])

    def test_graph_validation_rejects_orphan_markers_jobs_and_cache_refs(self):
        project = new_project("Demo")
        source = import_audio_asset_from_bytes(project, b"audio")

        project.markers.append(Marker(id="marker_orphan", track_id="missing", timestamp=0.0))
        with self.assertRaisesRegex(ValueError, "marker references missing track"):
            validate_graph(project)
        project.markers.clear()

        project.job_runs.append(JobRun(id="job_orphan", track_id="missing", transform_id="x", parameters_hash="h"))
        with self.assertRaisesRegex(ValueError, "job run references missing track"):
            validate_graph(project)
        project.job_runs.clear()

        source.cache_refs.append("entry_missing")
        with self.assertRaisesRegex(ValueError, "track cache ref not found"):
            validate_graph(project)

        project.cache_entries.append(
            CacheEntry(
                id="entry_missing",
                dependency_hash="dep",
                artifact_kind="stem",
                path="stem/entry_missing.bin",
                created_at="",
                transform_version="1",
            )
        )
        validate_graph(project)

    def test_graph_validation_rejects_source_tracks_without_audio_asset(self):
        project = new_project("Demo")
        source = import_audio_asset_from_bytes(project, b"audio")

        source.provenance["asset_id"] = "missing_asset"

        with self.assertRaisesRegex(ValueError, "source track references missing audio asset"):
            validate_graph(project)

    def test_graph_validation_rejects_duplicate_audio_asset_ids(self):
        project = new_project("Demo")
        import_audio_asset_from_bytes(project, b"audio")
        project.audio_assets.append(
            AudioAsset(
                id=project.audio_assets[0].id,
                path="/music/duplicate.wav",
                duration=0.0,
                sample_rate=0,
                channels=0,
                fingerprint="duplicate",
            )
        )

        with self.assertRaisesRegex(ValueError, "duplicate audio asset id"):
            validate_graph(project)


def import_audio_asset_from_bytes(project, payload: bytes):
    with tempfile.TemporaryDirectory() as tmp:
        audio_path = Path(tmp) / "song.wav"
        audio_path.write_bytes(payload)
        return import_audio_asset(project, audio_path)


if __name__ == "__main__":
    unittest.main()
