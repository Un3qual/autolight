import unittest

from PySide6.QtCore import QCoreApplication

from autolight.project.models import CacheEntry, ProjectDocument, Track, TrackType
from autolight.timeline.model import TimelineTrackModel


class StemArtifactWorkflowTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication([])

    def test_timeline_model_exposes_artifact_summary_roles(self):
        project = ProjectDocument(id="project_1", name="Demo")
        project.tracks.append(
            Track(
                id="track_stem",
                type=TrackType.GENERATED,
                name="Vocals",
                cache_refs=["cache_1"],
            )
        )
        project.cache_entries.append(
            CacheEntry(
                id="cache_1",
                dependency_hash="dep",
                artifact_kind="stem",
                path="stem/cache_1.json",
                created_at="",
                transform_version="1",
            )
        )
        model = TimelineTrackModel()
        model.set_project(project)
        index = model.index(0, 0)

        self.assertEqual(model.data(index, model.role_for_name("cacheRefCount")), 1)
        self.assertEqual(model.data(index, model.role_for_name("artifactKinds")), "stem")

    def test_controller_adds_vocals_stem_track(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        source_id = controller.trackModel.data(
            controller.trackModel.index(0, 0),
            controller.trackModel.role_for_name("trackId"),
        )

        stem_id = controller.add_vocals_stem_track(source_id)

        self.assertNotEqual(stem_id, "")
        stem = self._track_by_id(controller, stem_id)
        self.assertEqual(stem.transform_id, "stems.vocals_stand_in")
        self.assertEqual(stem.output_schema, "artifact.stem.v1")

    def test_qml_exposes_stem_workflow(self):
        from pathlib import Path

        ui_root = Path(__file__).resolve().parents[1] / "UI"
        qml = "\n".join(
            [
                (ui_root / "Main.qml").read_text(encoding="utf-8"),
                (ui_root / "components" / "TrackRow.qml").read_text(encoding="utf-8"),
            ]
        )
        self.assertIn("appController.add_vocals_stem_track(appController.selectedTrackId)", qml)
        self.assertIn("artifactKinds", qml)
        self.assertIn("cacheRefCount", qml)

    def _track_by_id(self, controller, track_id: str):
        for track in controller._project.tracks:
            if track.id == track_id:
                return track
        self.fail(f"track not found: {track_id}")


if __name__ == "__main__":
    unittest.main()
