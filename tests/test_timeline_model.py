import tempfile
import unittest
from pathlib import Path

from PySide6.QtCore import QCoreApplication, Qt

from autolight.project.models import Marker, ResultState
from autolight.project.store import add_generated_track, import_audio_asset, new_project
from autolight.timeline.model import TimelineTrackModel


class TimelineTrackModelTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication([])

    def test_model_exposes_track_roles_for_qml(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            audio_path.write_bytes(b"audio")
            project = new_project("Demo")
            source = import_audio_asset(project, audio_path)
            generated = add_generated_track(project, source.id, "Beats", "markers.fixed_interval", {}, "1", "markers.v1", "dep")
            generated.result_state = ResultState.COMPLETE
            project.markers.append(Marker(id="marker_1", track_id=generated.id, timestamp=0.5))

            model = TimelineTrackModel()
            model.set_project(project)
            index = model.index(1, 0)

            self.assertEqual(model.rowCount(), 2)
            self.assertEqual(model.data(index, model.role_for_name("name")), "Beats")
            self.assertEqual(model.data(index, model.role_for_name("markerCount")), 1)
            self.assertEqual(model.data(index, model.role_for_name("resultState")), "complete")
            self.assertEqual(model.data(index, Qt.ItemDataRole.DisplayRole), "Beats")


if __name__ == "__main__":
    unittest.main()
