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

    def _generated_track(self, project):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            source = import_audio_asset(project, audio_path)
        from autolight.project.store import add_generated_track

        return add_generated_track(project, source.id, "Generated", "markers.fixed_interval", {}, "1", "markers.v1", "hash")


if __name__ == "__main__":
    unittest.main()
