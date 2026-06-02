import unittest
import tempfile
from pathlib import Path

from PySide6.QtCore import QCoreApplication

from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformRegistry, TransformResult, TransformSpec
from autolight.project.models import ResultState, Track, TrackType
from autolight.timeline.transform_model import TransformSpecModel


class TransformPickerTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication([])

    def test_transform_model_exposes_registered_specs(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        model = TransformSpecModel(registry)

        ids = [
            model.data(model.index(row, 0), model.role_for_name("transformId"))
            for row in range(model.rowCount())
        ]

        self.assertIn("markers.fixed_interval", ids)
        self.assertIn("stems.vocals_stand_in", ids)

    def test_transform_model_exposes_multiple_versions_for_same_transform(self):
        def noop(context, params):
            return TransformResult()

        registry = TransformRegistry()
        registry.register(
            TransformSpec(
                id="test.versioned",
                version="1",
                name="Versioned 1",
                input_schema="audio.v1",
                output_schema="markers.v1",
                estimated_cost="light",
                run=noop,
            )
        )
        registry.register(
            TransformSpec(
                id="test.versioned",
                version="2",
                name="Versioned 2",
                input_schema="audio.v1",
                output_schema="markers.v1",
                estimated_cost="light",
                run=noop,
            )
        )

        model = TransformSpecModel(registry)
        versions = [
            model.data(model.index(row, 0), model.role_for_name("version"))
            for row in range(model.rowCount())
        ]

        self.assertEqual(versions, ["1", "2"])

    def test_controller_add_transform_track_accepts_json_params(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        source_id = controller.trackModel.data(
            controller.trackModel.index(0, 0),
            controller.trackModel.role_for_name("trackId"),
        )

        track_id = controller.add_transform_track(
            source_id,
            "markers.fixed_interval",
            "1",
            '{"duration": 3.0, "interval": 1.0}',
        )

        self.assertNotEqual(track_id, "")
        track = self._track_by_id(controller, track_id)
        self.assertEqual(track.transform_id, "markers.fixed_interval")
        self.assertEqual(track.transform_version, "1")
        self.assertEqual(track.transform_params, {"duration": 3.0, "interval": 1.0})

    def test_controller_add_transform_track_defaults_audio_path_for_audio_transform(self):
        from autolight.app_controller import AppController

        def noop(context, params):
            return TransformResult()

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller._registry.register(
            TransformSpec(
                id="test.audio_path",
                version="1",
                name="Audio Path Transform",
                input_schema="audio.v1",
                output_schema="markers.v1",
                estimated_cost="light",
                run=noop,
            )
        )
        controller.load_demo_project()
        source_id = controller.trackModel.data(
            controller.trackModel.index(0, 0),
            controller.trackModel.role_for_name("trackId"),
        )

        track_id = controller.add_transform_track(source_id, "test.audio_path", "1", "{}")

        track = self._track_by_id(controller, track_id)
        self.assertNotIn("audio_path", track.transform_params)

    def test_controller_add_transform_track_rejects_generated_marker_audio_parent(self):
        from autolight.app_controller import AppController

        def noop(context, params):
            return TransformResult()

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller._registry.register(
            TransformSpec(
                id="test.audio_path",
                version="1",
                name="Audio Path Transform",
                input_schema="audio.v1",
                output_schema="markers.v1",
                estimated_cost="light",
                run=noop,
            )
        )
        controller.load_demo_project()
        source_id = controller.trackModel.data(
            controller.trackModel.index(0, 0),
            controller.trackModel.role_for_name("trackId"),
        )
        generated_id = controller.add_transform_track(
            source_id,
            "markers.fixed_interval",
            "1",
            '{"duration": 3.0, "interval": 1.0}',
        )
        generated = self._track_by_id(controller, generated_id)
        generated.result_state = ResultState.COMPLETE

        track_id = controller.add_transform_track(generated_id, "test.audio_path", "1", "{}")

        self.assertEqual(track_id, "")
        self.assertIn("parent track has no valid audio artifact", controller.lastError)

    def test_controller_add_transform_track_routes_complete_editable_audio_parent_to_source(self):
        from autolight.app_controller import AppController

        def noop(context, params):
            return TransformResult()

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller._registry.register(
            TransformSpec(
                id="test.audio_path",
                version="1",
                name="Audio Path Transform",
                input_schema="audio.v1",
                output_schema="markers.v1",
                estimated_cost="light",
                run=noop,
            )
        )
        controller.load_demo_project()
        source_id = controller.trackModel.data(
            controller.trackModel.index(0, 0),
            controller.trackModel.role_for_name("trackId"),
        )
        no_audio = Track(id="track_no_audio", type=TrackType.EDITABLE, name="No Audio")
        multi_parent = Track(
            id="track_multi_parent",
            type=TrackType.EDITABLE,
            name="Editable Multi Parent",
            input_track_ids=[no_audio.id, source_id],
            result_state=ResultState.COMPLETE,
        )
        controller._project.tracks.extend([no_audio, multi_parent])

        track_id = controller.add_transform_track(multi_parent.id, "test.audio_path", "1", "{}")

        track = self._track_by_id(controller, track_id)
        self.assertNotIn("audio_path", track.transform_params)

    def test_controller_add_transform_track_rejects_audio_transform_without_source_audio(self):
        from autolight.app_controller import AppController

        def noop(context, params):
            return TransformResult()

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller._registry.register(
            TransformSpec(
                id="test.audio_path",
                version="1",
                name="Audio Path Transform",
                input_schema="audio.v1",
                output_schema="markers.v1",
                estimated_cost="light",
                run=noop,
            )
        )
        controller.load_demo_project()
        no_audio = Track(id="track_no_audio", type=TrackType.EDITABLE, name="No Audio")
        controller._project.tracks.append(no_audio)

        track_id = controller.add_transform_track(no_audio.id, "test.audio_path", "1", "{}")

        self.assertEqual(track_id, "")
        self.assertIn("parent track is not complete", controller.lastError)

    def test_controller_add_transform_track_rejects_supplied_audio_path_without_source_audio(self):
        from autolight.app_controller import AppController

        def noop(context, params):
            return TransformResult()

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller._registry.register(
            TransformSpec(
                id="test.audio_path",
                version="1",
                name="Audio Path Transform",
                input_schema="audio.v1",
                output_schema="markers.v1",
                estimated_cost="light",
                run=noop,
            )
        )
        controller.load_demo_project()
        no_audio = Track(id="track_no_audio", type=TrackType.EDITABLE, name="No Audio")
        controller._project.tracks.append(no_audio)

        track_id = controller.add_transform_track(
            no_audio.id,
            "test.audio_path",
            "1",
            '{"audio_path": "/tmp/other.wav"}',
        )

        self.assertEqual(track_id, "")
        self.assertIn("parent track is not complete", controller.lastError)

    def test_controller_resolves_audio_path_at_submission_time(self):
        from autolight.app_controller import AppController
        from tests.helpers import write_wav

        seen_paths = []

        def capture_path(context, params):
            seen_paths.append(params["audio_path"])
            return TransformResult()

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller._registry.register(
            TransformSpec(
                id="test.runtime_audio_path",
                version="1",
                name="Runtime Audio Path Transform",
                input_schema="audio.v1",
                output_schema="markers.v1",
                estimated_cost="light",
                run=capture_path,
            )
        )

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            original_path = root / "song.wav"
            relinked_path = root / "song_relinked.wav"
            write_wav(original_path)
            write_wav(relinked_path)
            source_id = controller.import_audio(str(original_path))
            track_id = controller.add_transform_track(source_id, "test.runtime_audio_path", "1", "{}")
            track = self._track_by_id(controller, track_id)
            controller._project.audio_assets[0].path = str(relinked_path)

            job_id = controller.run_track(track_id)
            controller._job_queue.wait(job_id, timeout=5)
            QCoreApplication.processEvents()

        self.assertEqual(seen_paths, [str(relinked_path)])
        self.assertNotIn("audio_path", track.transform_params)

    def test_controller_runtime_audio_path_replaces_saved_audio_path(self):
        from autolight.app_controller import AppController
        from tests.helpers import write_wav

        seen_paths = []

        def capture_path(context, params):
            seen_paths.append(params["audio_path"])
            return TransformResult()

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller._registry.register(
            TransformSpec(
                id="test.runtime_audio_path",
                version="1",
                name="Runtime Audio Path Transform",
                input_schema="audio.v1",
                output_schema="markers.v1",
                estimated_cost="light",
                run=capture_path,
            )
        )

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            original_path = root / "song.wav"
            relinked_path = root / "song_relinked.wav"
            write_wav(original_path)
            write_wav(relinked_path)
            source_id = controller.import_audio(str(original_path))
            track_id = controller.add_transform_track(
                source_id,
                "test.runtime_audio_path",
                "1",
                '{"audio_path": "/stale/song.wav"}',
            )
            track = self._track_by_id(controller, track_id)
            controller._project.audio_assets[0].path = str(relinked_path)

            job_id = controller.run_track(track_id)
            controller._job_queue.wait(job_id, timeout=5)
            QCoreApplication.processEvents()

        self.assertEqual(seen_paths, [str(relinked_path)])
        self.assertNotIn("audio_path", track.transform_params)

    def test_controller_add_vocals_stem_track_rejects_parent_without_source_audio(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        no_audio = Track(id="track_no_audio", type=TrackType.EDITABLE, name="No Audio")
        controller._project.tracks.append(no_audio)

        track_id = controller.add_vocals_stem_track(no_audio.id)

        self.assertEqual(track_id, "")
        self.assertIn("parent track is not complete", controller.lastError)

    def test_qml_uses_transform_model_and_generic_add_action(self):
        ui_root = Path(__file__).resolve().parents[1] / "UI"
        qml = "\n".join(
            [
                (ui_root / "Main.qml").read_text(encoding="utf-8"),
                (ui_root / "components" / "TransformBar.qml").read_text(encoding="utf-8"),
            ]
        )
        self.assertIn("model: root.appController.transformModel", qml)
        self.assertIn("textRole: \"name\"", qml)
        self.assertIn("appController.add_transform_track(", qml)
        self.assertIn("root.appController.transformModel.version_at(transformPicker.currentIndex)", qml)
        self.assertIn("transformParamsField.text", qml)

    def _track_by_id(self, controller, track_id: str):
        for track in controller._project.tracks:
            if track.id == track_id:
                return track
        self.fail(f"track not found: {track_id}")


if __name__ == "__main__":
    unittest.main()
