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
                    model.role_for_name("error"): b"error",
                },
            )
            self.assertEqual(model.rowCount(), 2)
            self.assertEqual(model.data(index, model.role_for_name("trackId")), generated.id)
            self.assertEqual(model.data(index, model.role_for_name("name")), "Beats")
            self.assertEqual(model.data(index, model.role_for_name("trackType")), "generated")
            self.assertEqual(model.data(index, model.role_for_name("markerCount")), 1)
            self.assertEqual(model.data(index, model.role_for_name("resultState")), "complete")
            self.assertEqual(model.data(index, model.role_for_name("error")), "analysis failed")
            self.assertEqual(model.data(index, Qt.ItemDataRole.DisplayRole), "Beats")

    def test_role_names_returns_copy(self):
        model = TimelineTrackModel()
        role_names = model.roleNames()

        role_names[model.role_for_name("name")] = b"changed"

        self.assertEqual(model.roleNames()[model.role_for_name("name")], b"name")

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

    def test_constructor_accepts_optional_parent(self):
        parent = QCoreApplication.instance()

        model = TimelineTrackModel(parent=parent)

        self.assertIs(model.parent(), parent)

    def _project_with_generated_track(self, tmp: Path):
        audio_path = tmp / "song.wav"
        audio_path.write_bytes(b"audio")
        project = new_project("Demo")
        source = import_audio_asset(project, audio_path)
        generated = add_generated_track(
            project,
            source.id,
            "Beats",
            "markers.fixed_interval",
            {},
            "1",
            "markers.v1",
            "dep",
        )
        return project, source, generated


if __name__ == "__main__":
    unittest.main()
