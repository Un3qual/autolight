import math
import tempfile
import unittest
import wave
from pathlib import Path

from autolight.project.models import Marker
from autolight.project.store import (
    add_editable_marker,
    create_editable_track_from_markers,
    delete_editable_marker,
    import_audio_asset,
    new_project,
)


def write_wav(path: Path) -> None:
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(8000)
        handle.writeframes(b"\0\0" * 8000)


class EditableMarkerInspectorTest(unittest.TestCase):
    def test_add_editable_marker_rejects_generated_track(self):
        project = new_project("Demo")
        generated = self._generated_track(project)

        with self.assertRaisesRegex(ValueError, "editable track"):
            add_editable_marker(project, generated.id, 1.0, "Cue")

    def test_add_and_delete_editable_marker(self):
        project = new_project("Demo")
        generated = self._generated_track(project)
        project.markers.append(Marker(id="marker_source", track_id=generated.id, timestamp=0.5))
        editable = create_editable_track_from_markers(project, generated.id, "Editable", ["marker_source"])

        marker = add_editable_marker(project, editable.id, 1.25, "Cue")
        deleted = delete_editable_marker(project, editable.id, marker.id)

        self.assertTrue(deleted)
        self.assertNotIn(marker.id, [item.id for item in project.markers if item.track_id == editable.id])

    def test_add_editable_marker_rejects_non_finite_timestamp(self):
        project = new_project("Demo")
        generated = self._generated_track(project)
        project.markers.append(Marker(id="marker_source", track_id=generated.id, timestamp=0.5))
        editable = create_editable_track_from_markers(project, generated.id, "Editable", ["marker_source"])

        with self.assertRaisesRegex(ValueError, "finite"):
            add_editable_marker(project, editable.id, math.nan, "Cue")

        with self.assertRaisesRegex(ValueError, "finite"):
            add_editable_marker(project, editable.id, math.inf, "Cue")

    def test_controller_adds_marker_to_selected_editable_track(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)

        marker_id = controller.add_marker_to_selected_track(1.5, "Blackout")

        self.assertNotEqual(marker_id, "")
        self.assertEqual(controller.lastError, "")
        self.assertTrue(any(marker.id == marker_id for marker in controller._project.markers))

    def test_controller_rejects_non_finite_marker_timestamp(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)

        marker_id = controller.add_marker_to_selected_track(math.nan, "Broken")

        self.assertEqual(marker_id, "")
        self.assertIn("finite", controller.lastError)

        marker_id = controller.add_marker_to_selected_track(math.inf, "Broken")

        self.assertEqual(marker_id, "")
        self.assertIn("finite", controller.lastError)

    def test_controller_deletes_marker_from_selected_editable_track(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        marker_id = controller.add_marker_to_selected_track(1.5, "Blackout")

        self.assertTrue(controller.delete_marker_from_selected_track(marker_id))

        self.assertFalse(any(marker.id == marker_id for marker in controller._project.markers))
        self.assertEqual(controller.lastError, "")

    def _generated_track(self, project):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            source = import_audio_asset(project, audio_path)
        from autolight.project.store import add_generated_track

        return add_generated_track(project, source.id, "Generated", "markers.fixed_interval", {}, "1", "markers.v1", "hash")

    def _track_id_for_type(self, controller, track_type: str) -> str:
        model = controller.trackModel
        type_role = model.role_for_name("trackType")
        id_role = model.role_for_name("trackId")
        for row in range(model.rowCount()):
            index = model.index(row, 0)
            if model.data(index, type_role) == track_type:
                return model.data(index, id_role)
        raise AssertionError(f"track type not found: {track_type}")


if __name__ == "__main__":
    unittest.main()
