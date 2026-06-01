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


if __name__ == "__main__":
    unittest.main()
