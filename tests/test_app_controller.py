import unittest
import tempfile
from pathlib import Path
from unittest.mock import patch

from PySide6.QtCore import QCoreApplication

import main as app_entry
from autolight.app_controller import AppController
from autolight.cache.keys import track_dependency_hash
from autolight.project.models import CacheEntry, JobRun, ResultState
from tests.helpers import write_wav


class FakeContext:
    def __init__(self):
        self.properties = {}

    def setContextProperty(self, name, value):
        self.properties[name] = value


class FakeEngine:
    instances = []
    root_objects = [object()]

    def __init__(self):
        self.context = FakeContext()
        self.import_paths = []
        self.loaded_modules = []
        type(self).instances.append(self)

    def rootContext(self):
        return self.context

    def addImportPath(self, path):
        self.import_paths.append(path)

    def loadFromModule(self, module, name):
        self.loaded_modules.append((module, name))

    def rootObjects(self):
        return type(self).root_objects


class FakeGuiApplication:
    def __init__(self, args):
        self.args = args
        self.exec_called = False

    def exec(self):
        self.exec_called = True
        raise AssertionError("smoke mode must not enter the Qt event loop")


class AppControllerTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication([])

    def test_controller_loads_demo_project_into_timeline_model(self):
        controller = self._controller()

        controller.load_demo_project()

        self.assertEqual(controller.trackModel.rowCount(), 3)
        self.assertEqual(controller.projectName, "Autolight Demo")

    def test_controller_emits_project_name_changed_when_demo_loads(self):
        controller = self._controller()
        project_names = []
        controller.projectNameChanged.connect(lambda: project_names.append(controller.projectName))

        controller.load_demo_project()

        self.assertEqual(project_names, ["Autolight Demo"])

    def test_controller_parents_track_model(self):
        controller = self._controller()

        self.assertIs(controller.trackModel.parent(), controller)

    def test_controller_demo_project_exposes_expected_track_roles(self):
        controller = self._controller()

        controller.load_demo_project()

        self.assertEqual(
            [self._track_role_values(controller, row) for row in range(controller.trackModel.rowCount())],
            [
                {
                    "name_prefix": "autolight-demo-",
                    "trackType": "source",
                    "resultState": "complete",
                    "markerCount": 0,
                },
                {
                    "name": "Beat Markers",
                    "trackType": "generated",
                    "resultState": "complete",
                    "markerCount": 3,
                },
                {
                    "name": "Editable Cues",
                    "trackType": "editable",
                    "resultState": "complete",
                    "markerCount": 2,
                },
            ],
        )

    def test_controller_uses_unique_demo_audio_paths(self):
        controller = self._controller()

        controller.load_demo_project()
        first_path = controller._project.audio_assets[0].path
        controller.load_demo_project()
        second_path = controller._project.audio_assets[0].path

        self.assertNotEqual(first_path, second_path)

    def test_new_project_resets_project_path_and_timeline_model(self):
        controller = self._controller()
        controller.load_demo_project()

        controller.new_project()

        self.assertEqual(controller.projectName, "Untitled")
        self.assertEqual(controller.projectPath, "")
        self.assertEqual(controller.lastError, "")
        self.assertEqual(controller.trackModel.rowCount(), 0)
        self.assertFalse(controller.isDirty)

    def test_import_audio_adds_source_track_and_selects_it(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            track_id = controller.import_audio(str(audio_path))

        self.assertNotEqual(track_id, "")
        self.assertEqual(controller.trackModel.rowCount(), 1)
        self.assertEqual(controller.selectedTrackId, track_id)
        self.assertEqual(controller.lastError, "")
        self.assertTrue(controller.isDirty)

    def test_import_audio_records_error_for_missing_file(self):
        controller = self._controller()

        track_id = controller.import_audio("/missing/song.wav")

        self.assertEqual(track_id, "")
        self.assertIn("No such file", controller.lastError)
        self.assertEqual(controller.trackModel.rowCount(), 0)

    def test_save_and_open_project_round_trip_updates_path_and_model(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            write_wav(audio_path)
            project_path = root / "show.autolight"
            controller.import_audio(str(audio_path))

            self.assertTrue(controller.save_project(str(project_path)))
            controller.new_project()
            self.assertTrue(controller.open_project(str(project_path)))

        self.assertEqual(controller.projectName, "Untitled")
        self.assertTrue(controller.projectPath.endswith("show.autolight"))
        self.assertEqual(controller.trackModel.rowCount(), 1)
        self.assertEqual(controller.lastError, "")
        self.assertFalse(controller.isDirty)

    def test_save_project_requires_path_for_unsaved_project(self):
        controller = self._controller()

        self.assertFalse(controller.save_project(""))
        self.assertIn("project path is required", controller.lastError)

    def test_open_project_marks_missing_cache_artifacts_stale(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            write_wav(audio_path)
            project_path = root / "show.autolight"
            source_id = controller.import_audio(str(audio_path))
            generated_id = controller.add_fixed_interval_track(source_id, 2.0, 0.5)
            generated = self._track_by_id(controller, generated_id)
            generated.result_state = ResultState.COMPLETE
            generated.cache_refs = ["cache_missing"]
            controller._project.cache_entries.append(
                CacheEntry(
                    id="cache_missing",
                    dependency_hash=generated.dependency_hash,
                    artifact_kind="stem",
                    path="missing/stem.json",
                    created_at="2026-05-31T00:00:00+00:00",
                    transform_version="1",
                    size_bytes=1,
                    payload_digest="missing-digest",
                )
            )
            self.assertTrue(controller.save_project(str(project_path)))

            reopened = self._controller()
            self.assertTrue(reopened.open_project(str(project_path)))

        reopened_generated = self._track_by_id(reopened, generated_id)
        reopened_cache_entry = reopened._project.cache_entries[0]
        self.assertEqual(reopened_generated.result_state, ResultState.STALE)
        self.assertEqual(reopened_cache_entry.validation_status, "invalid")
        self.assertIn("cache artifact", reopened_generated.error)
        self.assertTrue(reopened.isDirty)

    def test_open_project_refreshes_missing_audio_asset_status(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            write_wav(audio_path)
            project_path = root / "show.autolight"
            controller.import_audio(str(audio_path))
            self.assertTrue(controller.save_project(str(project_path)))
            audio_path.unlink()

            reopened = self._controller()
            self.assertTrue(reopened.open_project(str(project_path)))

        asset = reopened._project.audio_assets[0]
        self.assertEqual(asset.import_status, "offline")
        self.assertEqual(asset.relink_hint, "song.wav")
        self.assertTrue(reopened.isDirty)

    def test_open_project_searches_project_folder_for_relinked_audio(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            relinked_path = root / "song-copy.wav"
            write_wav(audio_path)
            project_path = root / "show.autolight"
            controller.import_audio(str(audio_path))
            self.assertTrue(controller.save_project(str(project_path)))
            relinked_path.write_bytes(audio_path.read_bytes())
            audio_path.unlink()

            reopened = self._controller()
            self.assertTrue(reopened.open_project(str(project_path)))

        asset = reopened._project.audio_assets[0]
        self.assertEqual(asset.path, str(relinked_path))
        self.assertEqual(asset.import_status, "online")
        self.assertEqual(asset.relink_hint, "")
        self.assertTrue(reopened.isDirty)

    def test_refresh_cache_status_marks_invalid_cached_track_stale(self):
        controller = self._controller()
        controller.load_demo_project()
        generated = controller._project.tracks[1]
        generated.result_state = ResultState.COMPLETE
        generated.cache_refs = ["missing_cache"]
        controller._project.cache_entries.append(
            CacheEntry(
                id="missing_cache",
                dependency_hash="dep",
                artifact_kind="stem",
                path="stem/missing.bin",
                created_at="",
                transform_version="1",
                size_bytes=10,
            )
        )

        invalid_refs = controller.refresh_cache_status()

        self.assertEqual(invalid_refs, ["missing_cache"])
        self.assertEqual(generated.result_state, ResultState.STALE)
        self.assertIn("cache artifact", generated.error)
        self.assertIn("invalid cache artifacts: 1", controller.lastError)

    def test_save_project_rejects_running_jobs(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self._add_running_job(controller, root)
            project_path = root / "show.autolight"

            self.assertFalse(controller.save_project(str(project_path)))
            self.assertFalse(project_path.exists())

        self.assertIn("running job", controller.lastError)

    def test_project_replacement_rejects_running_jobs(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self._add_running_job(controller, root)
            row_count = controller.trackModel.rowCount()
            replacement = self._controller()
            replacement.load_demo_project()
            replacement_path = root / "replacement.autolight"
            self.assertTrue(replacement.save_project(str(replacement_path)))

            controller.new_project()
            self.assertEqual(controller.projectName, "Untitled")
            self.assertEqual(controller.trackModel.rowCount(), row_count)
            self.assertIn("cannot replace project", controller.lastError)

            self.assertFalse(controller.open_project(str(replacement_path)))
            self.assertEqual(controller.projectName, "Untitled")
            self.assertEqual(controller.trackModel.rowCount(), row_count)
            self.assertIn("cannot replace project", controller.lastError)

            controller.load_demo_project()
            self.assertEqual(controller.projectName, "Untitled")
            self.assertEqual(controller.trackModel.rowCount(), row_count)
            self.assertIn("cannot replace project", controller.lastError)

    def test_select_track_updates_selected_track_id(self):
        controller = self._controller()
        controller.load_demo_project()
        second_track_id = self._track_id(controller, 1)

        controller.select_track(second_track_id)

        self.assertEqual(controller.selectedTrackId, second_track_id)

    def test_selected_track_can_rerun_only_for_transform_tracks(self):
        controller = self._controller()
        controller.load_demo_project()

        self.assertFalse(controller.selectedTrackCanRerun)

        controller.select_track(self._track_id(controller, 1))
        self.assertTrue(controller.selectedTrackCanRerun)

        controller.select_track(self._track_id(controller, 2))
        self.assertFalse(controller.selectedTrackCanRerun)

    def test_selected_track_is_editable_only_for_editable_tracks(self):
        controller = self._controller()
        controller.load_demo_project()

        self.assertFalse(controller.selectedTrackIsEditable)

        controller.select_track(self._track_id(controller, 1))
        self.assertFalse(controller.selectedTrackIsEditable)

        controller.select_track(self._track_id(controller, 2))
        self.assertTrue(controller.selectedTrackIsEditable)

    def test_selected_track_has_running_job_follows_job_state(self):
        from threading import Event

        from autolight.analysis.registry import TransformCancelled, TransformResult, TransformSpec
        from autolight.project.store import add_generated_track

        started = Event()
        release = Event()

        def blocking_transform(context, params):
            started.set()
            while not release.wait(0.01):
                if context.cancel_requested():
                    raise TransformCancelled()
            if context.cancel_requested():
                raise TransformCancelled()
            return TransformResult()

        controller = self._controller()
        controller.load_demo_project()
        controller._registry.register(
            TransformSpec(
                id="test.blocking_selected_job",
                version="1",
                name="Blocking Selected Job Test",
                input_schema="audio.v1",
                output_schema="artifact.test.v1",
                estimated_cost="light",
                run=blocking_transform,
            )
        )
        source_id = self._track_id(controller, 0)
        generated = add_generated_track(
            controller._project,
            source_id,
            "Blocking Track",
            "test.blocking_selected_job",
            {},
            "1",
            "artifact.test.v1",
            "blocking_dependency",
        )
        controller.trackModel.set_project(controller._project)
        controller.select_track(generated.id)

        self.assertFalse(controller.selectedTrackHasRunningJob)
        job_id = controller.run_track(generated.id)
        try:
            self.assertNotEqual(job_id, "")
            self.assertTrue(started.wait(2))
            self.assertTrue(controller.selectedTrackHasRunningJob)
            controller.cancel_selected_job()
            controller._job_queue.wait(job_id, timeout=2)
        finally:
            release.set()

        self.assertFalse(controller.selectedTrackHasRunningJob)

    def test_add_fixed_interval_track_uses_parent_and_selects_generated_track(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            source_id = controller.import_audio(str(audio_path))
            generated_id = controller.add_fixed_interval_track(source_id, 2.0, 0.5)

        self.assertNotEqual(generated_id, "")
        self.assertEqual(controller.trackModel.rowCount(), 2)
        self.assertEqual(controller.selectedTrackId, generated_id)
        generated = self._track_by_id(controller, generated_id)
        self.assertEqual(generated.input_track_ids, [source_id])
        self.assertEqual(generated.transform_id, "markers.fixed_interval")
        self.assertEqual(generated.transform_params, {"duration": 2.0, "interval": 0.5})
        self.assertNotEqual(generated.dependency_hash, "")

    def test_run_track_records_error_for_non_transform_track(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            source_id = controller.import_audio(str(audio_path))
            job_id = controller.run_track(source_id)

        self.assertEqual(job_id, "")
        self.assertIn("no transform", controller.lastError)

    def test_cancel_selected_job_cancels_running_track(self):
        from threading import Event

        from autolight.analysis.registry import TransformCancelled, TransformResult, TransformSpec
        from autolight.project.store import add_generated_track

        started = Event()
        release = Event()

        def blocking_transform(context, params):
            started.set()
            while not release.wait(0.01):
                if context.cancel_requested():
                    raise TransformCancelled()
            if context.cancel_requested():
                raise TransformCancelled()
            return TransformResult()

        controller = self._controller()
        controller.load_demo_project()
        controller._registry.register(
            TransformSpec(
                id="test.blocking_cancel",
                version="1",
                name="Blocking Cancel Test",
                input_schema="audio.v1",
                output_schema="artifact.test.v1",
                estimated_cost="light",
                run=blocking_transform,
            )
        )
        source_id = self._track_id(controller, 0)
        stem = add_generated_track(
            controller._project,
            source_id,
            "Vocals Stem",
            "test.blocking_cancel",
            {"label": "vocals"},
            "1",
            "artifact.stem.v1",
            "stem_dependency",
        )
        controller.trackModel.set_project(controller._project)
        controller.select_track(stem.id)

        job_id = controller.run_track(stem.id)
        self.assertNotEqual(job_id, "")
        try:
            self.assertTrue(started.wait(2))
            controller.cancel_selected_job()
            controller._job_queue.wait(job_id, timeout=2)
        finally:
            release.set()

        self.assertEqual(stem.result_state.value, "cancelled")

    def test_rerun_track_submits_existing_transform(self):
        controller = self._controller()
        controller.load_demo_project()
        generated_id = self._track_id(controller, 1)
        generated = self._track_by_id(controller, generated_id)
        generated.result_state = ResultState.STALE
        generated.error = "cache artifact missing or invalid: cache_1"

        job_id = controller.rerun_track(generated_id)
        controller._job_queue.wait(job_id, timeout=2)

        self.assertNotEqual(job_id, "")
        self.assertEqual(generated.result_state.value, "complete")

    def test_rerun_track_does_not_clear_stale_state_when_submit_fails(self):
        controller = self._controller()
        controller.load_demo_project()
        editable_id = self._track_id(controller, 2)
        editable = self._track_by_id(controller, editable_id)
        editable.result_state = ResultState.STALE
        editable.error = "source track changed"

        job_id = controller.rerun_track(editable_id)

        self.assertEqual(job_id, "")
        self.assertEqual(editable.result_state, ResultState.STALE)
        self.assertEqual(editable.error, "source track changed")
        self.assertIn("no transform", controller.lastError)

    def test_rerun_track_recomputes_dependency_hash_from_parent_cache_refs(self):
        from autolight.project.store import add_generated_track

        controller = self._controller()
        controller.load_demo_project()
        parent = self._track_by_id(controller, self._track_id(controller, 1))
        parent.cache_refs = ["cache_new"]
        child = add_generated_track(
            controller._project,
            parent.id,
            "Derived",
            "markers.fixed_interval",
            {"duration": 1.0, "interval": 0.5},
            "1",
            "markers.v1",
            "old_dependency_hash",
        )
        child.result_state = ResultState.STALE
        expected_hash = track_dependency_hash(
            parent.cache_refs,
            child.transform_id,
            child.transform_version,
            child.transform_params,
        )

        job_id = controller.rerun_track(child.id)
        controller._job_queue.wait(job_id, timeout=2)

        self.assertNotEqual(job_id, "")
        self.assertEqual(child.dependency_hash, expected_hash)

    def test_run_track_recomputes_dependency_hash_from_parent_cache_refs(self):
        from autolight.project.store import add_generated_track

        controller = self._controller()
        controller.load_demo_project()
        parent = self._track_by_id(controller, self._track_id(controller, 1))
        parent.cache_refs = ["cache_new"]
        child = add_generated_track(
            controller._project,
            parent.id,
            "Derived",
            "markers.fixed_interval",
            {"duration": 1.0, "interval": 0.5},
            "1",
            "markers.v1",
            "old_dependency_hash",
        )
        expected_hash = track_dependency_hash(
            parent.cache_refs,
            child.transform_id,
            child.transform_version,
            child.transform_params,
        )

        job_id = controller.run_track(child.id)
        controller._job_queue.wait(job_id, timeout=2)

        self.assertNotEqual(job_id, "")
        self.assertEqual(child.dependency_hash, expected_hash)

    def test_selected_track_markers_changed_emits_when_selected_job_updates_markers(self):
        controller = self._controller()
        controller.load_demo_project()
        generated_id = self._track_id(controller, 1)
        controller.select_track(generated_id)
        marker_changes = []
        controller.selectedTrackMarkersChanged.connect(lambda: marker_changes.append(controller.selectedTrackMarkers))

        job_id = controller.rerun_track(generated_id)
        controller._job_queue.wait(job_id, timeout=2)
        QCoreApplication.processEvents()

        self.assertGreaterEqual(len(marker_changes), 1)

    def test_create_editable_track_from_missing_track_records_not_found_error(self):
        controller = self._controller()

        editable_id = controller.create_editable_track_from_track("missing_track")

        self.assertEqual(editable_id, "")
        self.assertIn("track not found", controller.lastError)

    def test_create_editable_track_from_generated_markers_selects_editable_track(self):
        controller = self._controller()
        controller.load_demo_project()
        generated_id = self._track_id(controller, 1)

        editable_id = controller.create_editable_track_from_track(generated_id)

        self.assertNotEqual(editable_id, "")
        self.assertEqual(controller.trackModel.rowCount(), 4)
        self.assertEqual(controller.selectedTrackId, editable_id)
        editable = self._track_by_id(controller, editable_id)
        self.assertEqual(editable.input_track_ids, [generated_id])
        self.assertEqual(editable.result_state, ResultState.COMPLETE)

    def test_smoke_loads_qml_before_returning(self):
        FakeEngine.instances = []
        FakeEngine.root_objects = [object()]

        with patch.object(app_entry, "QGuiApplication", FakeGuiApplication), patch.object(
            app_entry,
            "QQmlApplicationEngine",
            FakeEngine,
        ):
            exit_code = app_entry.main(["main.py", "--smoke"])

        self.assertEqual(exit_code, 0)
        self.assertEqual(len(FakeEngine.instances), 1)
        self.assertEqual(FakeEngine.instances[0].loaded_modules, [("UI", "Main")])
        self.assertIsInstance(FakeEngine.instances[0].context.properties["appController"], AppController)

    def test_smoke_fails_when_qml_root_does_not_load(self):
        FakeEngine.instances = []
        FakeEngine.root_objects = []

        with patch.object(app_entry, "QGuiApplication", FakeGuiApplication), patch.object(
            app_entry,
            "QQmlApplicationEngine",
            FakeEngine,
        ):
            exit_code = app_entry.main(["main.py", "--smoke"])

        self.assertEqual(exit_code, -1)

    def test_qml_timeline_shell_uses_one_row_oriented_list(self):
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")

        self.assertEqual(qml.count("ListView {"), 1)
        self.assertIn("id: timelineRows", qml)
        self.assertIn("model: markerSpans", qml)
        self.assertIn("modelData.timestamp", qml)
        self.assertIn("spacing: root.timelinePixelsPerSecond", qml)
        self.assertIn("anchors.leftMargin: root.timelineLeftPadding", qml)
        self.assertIn("modelData.timestamp * root.timelinePixelsPerSecond", qml)
        self.assertNotIn("spacing: 48", qml)
        self.assertNotIn("pixelsPerSecond: 96", qml)
        self.assertNotIn("model: markerCount", qml)
        self.assertNotIn("onContentYChanged", qml)
        self.assertNotIn("contentY =", qml)

    def test_qml_timeline_ruler_has_fixed_height(self):
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")

        self.assertIn("readonly property real timelineRulerHeight: 32", qml)
        self.assertIn("id: timelineRuler", qml)
        self.assertIn("Layout.minimumHeight: root.timelineRulerHeight", qml)
        self.assertIn("Layout.preferredHeight: root.timelineRulerHeight", qml)
        self.assertIn("Layout.maximumHeight: root.timelineRulerHeight", qml)

    def test_qml_exposes_project_workflow_actions(self):
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")

        self.assertIn("import QtQuick.Dialogs", qml)
        self.assertIn("id: openProjectDialog", qml)
        self.assertIn("id: saveProjectDialog", qml)
        self.assertIn("id: importAudioDialog", qml)
        self.assertIn("id: discardChangesDialog", qml)
        self.assertIn("appController.isDirty", qml)
        self.assertIn("root.newProjectWithConfirmation()", qml)
        self.assertIn("root.demoProjectWithConfirmation()", qml)
        self.assertIn("root.openProjectWithConfirmation(String(selectedFile))", qml)
        self.assertIn("appController.new_project()", qml)
        self.assertIn("appController.open_project(path)", qml)
        self.assertIn('discardChangesDialog.pendingAction = "demo"', qml)
        self.assertIn("appController.load_demo_project()", qml)
        self.assertIn("appController.save_project(String(selectedFile))", qml)
        self.assertIn("appController.import_audio(String(selectedFile))", qml)
        self.assertIn("readonly property real defaultMarkerDuration: 8.0", qml)
        self.assertIn("readonly property real defaultMarkerInterval: 0.5", qml)
        self.assertIn(
            "appController.add_fixed_interval_track(appController.selectedTrackId, root.defaultMarkerDuration, root.defaultMarkerInterval)",
            qml,
        )
        self.assertIn("appController.run_track(appController.selectedTrackId)", qml)
        self.assertIn("appController.create_editable_track_from_track(appController.selectedTrackId)", qml)
        self.assertIn("appController.select_track(trackId)", qml)
        self.assertIn("appController.lastError", qml)

    def test_qml_exposes_job_progress_controls(self):
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")

        self.assertIn("jobProgress", qml)
        self.assertIn("activeJobId", qml)
        self.assertIn("ProgressBar", qml)
        self.assertIn("appController.cancel_selected_job()", qml)
        self.assertIn("appController.rerun_track(appController.selectedTrackId)", qml)
        self.assertIn("enabled: appController.selectedTrackHasRunningJob", qml)
        self.assertIn(
            "enabled: appController.selectedTrackCanRerun && !appController.selectedTrackHasRunningJob",
            qml,
        )

    def test_qml_exposes_cache_refresh_and_rerun_recovery(self):
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")

        self.assertIn("appController.refresh_cache_status()", qml)
        self.assertIn("appController.rerun_track(appController.selectedTrackId)", qml)
        self.assertIn('resultState === "stale"', qml)
        self.assertIn('resultState === "failed"', qml)

    @staticmethod
    def _track_role_values(controller: AppController, row: int):
        model = controller.trackModel
        index = model.index(row, 0)
        values = {
            "trackType": model.data(index, model.role_for_name("trackType")),
            "resultState": model.data(index, model.role_for_name("resultState")),
            "markerCount": model.data(index, model.role_for_name("markerCount")),
        }
        name = model.data(index, model.role_for_name("name"))
        if values["trackType"] == "source":
            values["name_prefix"] = name[: len("autolight-demo-")]
        else:
            values["name"] = name
        return values

    @staticmethod
    def _track_id(controller: AppController, row: int) -> str:
        model = controller.trackModel
        return model.data(model.index(row, 0), model.role_for_name("trackId"))

    def _track_by_id(self, controller: AppController, track_id: str):
        for track in controller._project.tracks:
            if track.id == track_id:
                return track
        self.fail(f"track not found: {track_id}")

    def _add_running_job(self, controller: AppController, root: Path) -> str:
        audio_path = root / "song.wav"
        write_wav(audio_path)
        source_id = controller.import_audio(str(audio_path))
        generated_id = controller.add_fixed_interval_track(source_id, 2.0, 0.5)
        generated = self._track_by_id(controller, generated_id)
        generated.result_state = ResultState.RUNNING
        controller._project.job_runs.append(
            JobRun(
                id="job_running",
                track_id=generated_id,
                transform_id=generated.transform_id,
                parameters_hash=generated.dependency_hash,
                state=ResultState.RUNNING,
            )
        )
        return generated_id

    def _controller(self):
        controller = AppController()
        self.addCleanup(controller.cleanup)
        return controller


if __name__ == "__main__":
    unittest.main()
