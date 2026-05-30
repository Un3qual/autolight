import tempfile
import unittest
from pathlib import Path

from PySide6.QtCore import QCoreApplication, QModelIndex, Qt

from autolight.project.models import Marker, ResultState
from autolight.project.store import add_generated_track, import_audio_asset, new_project
from autolight.timeline.model import TimelineTrackModel


class TimelineTrackModelTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication([])

    def test_model_exposes_track_roles_for_qml(self):
        with tempfile.TemporaryDirectory() as tmp:
            project, _source, generated = self._project_with_generated_track(Path(tmp))
            generated.result_state = ResultState.COMPLETE
            generated.error = "analysis failed"
            project.markers.append(Marker(id="marker_1", track_id=generated.id, timestamp=0.5))

            model = TimelineTrackModel()
            model.set_project(project)
            index = model.index(1, 0)

            self.assertEqual(
                model.roleNames(),
                {
                    model.role_for_name("trackId"): b"trackId",
                    model.role_for_name("name"): b"name",
                    model.role_for_name("trackType"): b"trackType",
                    model.role_for_name("resultState"): b"resultState",
                    model.role_for_name("markerCount"): b"markerCount",
                    model.role_for_name("markerSpans"): b"markerSpans",
                    model.role_for_name("error"): b"error",
                },
            )
            self.assertEqual(model.rowCount(), 2)
            self.assertEqual(model.data(index, model.role_for_name("trackId")), generated.id)
            self.assertEqual(model.data(index, model.role_for_name("name")), "Beats")
            self.assertEqual(model.data(index, model.role_for_name("trackType")), "generated")
            self.assertEqual(model.data(index, model.role_for_name("markerCount")), 1)
            self.assertEqual(
                model.data(index, model.role_for_name("markerSpans")),
                [
                    {
                        "id": "marker_1",
                        "timestamp": 0.5,
                        "duration": 0.0,
                        "label": "",
                        "category": "",
                    }
                ],
            )
            self.assertEqual(model.data(index, model.role_for_name("resultState")), "complete")
            self.assertEqual(model.data(index, model.role_for_name("error")), "analysis failed")
            self.assertEqual(model.data(index, Qt.ItemDataRole.DisplayRole), "Beats")

    def test_marker_spans_are_sorted_by_timestamp_for_timeline_projection(self):
        with tempfile.TemporaryDirectory() as tmp:
            project, _source, generated = self._project_with_generated_track(Path(tmp))
            project.markers.extend(
                [
                    Marker(id="marker_late", track_id=generated.id, timestamp=3.0, duration=0.25, label="Late"),
                    Marker(id="marker_early", track_id=generated.id, timestamp=0.75, label="Early"),
                ]
            )
            model = TimelineTrackModel()
            model.set_project(project)

            spans = model.data(model.index(1, 0), model.role_for_name("markerSpans"))

            self.assertEqual([span["id"] for span in spans], ["marker_early", "marker_late"])
            self.assertEqual([span["timestamp"] for span in spans], [0.75, 3.0])
            self.assertEqual([span["duration"] for span in spans], [0.0, 0.25])

    def test_marker_roles_use_cached_track_index(self):
        with tempfile.TemporaryDirectory() as tmp:
            project, _source, generated = self._project_with_generated_track(Path(tmp))
            project.markers.append(Marker(id="marker_1", track_id=generated.id, timestamp=0.5))
            model = TimelineTrackModel()
            model.set_project(project)
            project.markers = RaisingMarkerList(project.markers)
            index = model.index(1, 0)

            self.assertEqual(model.data(index, model.role_for_name("markerCount")), 1)
            self.assertEqual(
                [span["id"] for span in model.data(index, model.role_for_name("markerSpans"))],
                ["marker_1"],
            )

    def test_refresh_track_rebuilds_cached_marker_index_for_track(self):
        with tempfile.TemporaryDirectory() as tmp:
            project, _source, generated = self._project_with_generated_track(Path(tmp))
            model = TimelineTrackModel()
            model.set_project(project)
            index = model.index(1, 0)
            self.assertEqual(model.data(index, model.role_for_name("markerCount")), 0)

            project.markers.append(Marker(id="marker_1", track_id=generated.id, timestamp=0.5))
            model.refresh_track(generated.id)

            self.assertEqual(model.data(index, model.role_for_name("markerCount")), 1)

    def test_refresh_track_emits_data_changed_for_existing_track(self):
        with tempfile.TemporaryDirectory() as tmp:
            project, _source, generated = self._project_with_generated_track(Path(tmp))
            model = TimelineTrackModel()
            model.set_project(project)
            emissions = []
            model.dataChanged.connect(
                lambda top_left, bottom_right, roles: emissions.append(
                    (top_left.row(), bottom_right.row(), roles)
                )
            )

            model.refresh_track(generated.id)

            self.assertEqual(len(emissions), 1)
            self.assertEqual(emissions[0][0:2], (1, 1))
            self.assertIn(model.role_for_name("resultState"), emissions[0][2])
            self.assertIn(model.role_for_name("markerSpans"), emissions[0][2])

    def test_role_names_returns_copy(self):
        model = TimelineTrackModel()
        role_names = model.roleNames()

        role_names[model.role_for_name("name")] = b"changed"

        self.assertEqual(model.roleNames()[model.role_for_name("name")], b"name")

    def test_role_lookup_uses_cached_reverse_map(self):
        model = TimelineTrackModel()
        name_role = model.role_for_name("name")
        model.ROLE_NAMES = RaisingRoleNames(model.ROLE_NAMES)

        self.assertEqual(model.role_for_name("name"), name_role)

    def test_invalid_indexes_return_none(self):
        with tempfile.TemporaryDirectory() as tmp:
            project, _source, _generated = self._project_with_generated_track(Path(tmp))
            model = TimelineTrackModel()
            model.set_project(project)

            self.assertIsNone(model.data(QModelIndex(), model.role_for_name("name")))
            self.assertIsNone(model.data(model.createIndex(0, 1), model.role_for_name("name")))
            self.assertIsNone(model.data(model.createIndex(model.rowCount(), 0), model.role_for_name("name")))

            other_model = TimelineTrackModel()
            other_model.set_project(project)
            self.assertIsNone(model.data(other_model.index(0, 0), model.role_for_name("name")))

            stale_index = model.index(1, 0)
            model.set_project(new_project("Empty"))
            self.assertIsNone(model.data(stale_index, model.role_for_name("name")))

    def test_same_row_stale_index_after_reset_returns_none(self):
        with tempfile.TemporaryDirectory() as tmp:
            project_a, _source_a, _generated_a = self._project_with_generated_track(
                Path(tmp) / "project_a",
                generated_name="Beats A",
            )
            project_b, _source_b, _generated_b = self._project_with_generated_track(
                Path(tmp) / "project_b",
                generated_name="Beats B",
            )
            model = TimelineTrackModel()
            name_role = model.role_for_name("name")

            model.set_project(project_a)
            stale_index = model.index(1, 0)

            model.set_project(project_b)

            self.assertIsNone(model.data(stale_index, name_role))
            self.assertEqual(model.data(model.index(1, 0), name_role), "Beats B")

    def test_constructor_accepts_optional_parent(self):
        parent = QCoreApplication.instance()

        model = TimelineTrackModel(parent=parent)

        self.assertIs(model.parent(), parent)

    def _project_with_generated_track(self, tmp: Path, generated_name: str = "Beats"):
        tmp.mkdir(parents=True, exist_ok=True)
        audio_path = tmp / "song.wav"
        audio_path.write_bytes(b"audio")
        project = new_project("Demo")
        source = import_audio_asset(project, audio_path)
        generated = add_generated_track(
            project,
            source.id,
            generated_name,
            "markers.fixed_interval",
            {},
            "1",
            "markers.v1",
            "dep",
        )
        return project, source, generated


class RaisingMarkerList(list):
    def __iter__(self):
        raise AssertionError("marker list should not be scanned")


class RaisingRoleNames(dict):
    def items(self):
        raise AssertionError("role names should not be scanned")


if __name__ == "__main__":
    unittest.main()
