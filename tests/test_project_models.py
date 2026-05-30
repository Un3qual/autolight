import unittest

from autolight.project.models import (
    AudioAsset,
    Marker,
    ProjectDocument,
    ResultState,
    Track,
    TrackType,
)


class ProjectModelsTest(unittest.TestCase):
    def test_project_defaults_are_serializable_domain_objects(self):
        project = ProjectDocument(id="project_1", name="Demo")

        self.assertEqual(project.schema_version, 1)
        self.assertEqual(project.audio_assets, [])
        self.assertEqual(project.tracks, [])
        self.assertEqual(project.markers, [])

    def test_track_uses_list_inputs_for_future_dag_compatibility(self):
        track = Track(
            id="track_pitch",
            type=TrackType.GENERATED,
            name="Pitch",
            input_track_ids=["track_vocals"],
            transform_id="pitch.basic",
            transform_version="1",
        )

        self.assertEqual(track.input_track_ids, ["track_vocals"])
        self.assertEqual(track.result_state, ResultState.PENDING)

    def test_editable_marker_keeps_source_marker_ids(self):
        marker = Marker(
            id="marker_edit_1",
            track_id="track_edit",
            timestamp=1.25,
            label="Cue",
            source_marker_ids=["marker_generated_1"],
            metadata={"color": "blue"},
        )

        self.assertEqual(marker.source_marker_ids, ["marker_generated_1"])
        self.assertEqual(marker.metadata["color"], "blue")

    def test_audio_asset_records_relinkable_source_file(self):
        asset = AudioAsset(
            id="asset_1",
            path="/music/song.wav",
            duration=12.5,
            sample_rate=44100,
            channels=2,
            fingerprint="abc123",
        )

        self.assertEqual(asset.import_status, "online")
        self.assertEqual(asset.relink_hint, "")


if __name__ == "__main__":
    unittest.main()
