import copy
import importlib
import json
import math
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock
from unittest.mock import patch

from PySide6.QtCore import QCoreApplication
from PySide6.QtGui import QColor, QImage

import main as app_entry
from autolight.app import (
    EditHistory,
    MarkerEditingService,
    ProjectSession,
    TimelineViewport,
    WaveformLodStore,
)
from autolight.app_controller import AppController
from autolight.cache.keys import track_dependency_hash
from autolight.project.models import CacheEntry, JobRun, ResultState, TrackType
from autolight.project.store import track_dependency_inputs
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

    @staticmethod
    def _qml_text(*relative_paths: str) -> str:
        root = Path(__file__).resolve().parents[1]
        return "\n".join((root / relative_path).read_text(encoding="utf-8") for relative_path in relative_paths)

    def test_app_layer_modules_exist_for_milestone_2_boundaries(self):
        module_names = [
            "autolight.app.session",
            "autolight.app.marker_editing",
            "autolight.app.edit_history",
            "autolight.app.timeline_viewport",
            "autolight.app.waveform_lod",
        ]

        for module_name in module_names:
            with self.subTest(module_name=module_name):
                self.assertIsNotNone(importlib.import_module(module_name))

    def test_plain_app_import_defers_project_store_dependencies(self):
        script = "\n".join(
            [
                "import sys",
                "import autolight.app",
                "project_modules = sorted(",
                "    name for name in sys.modules if name.startswith('autolight.project')",
                ")",
                "if project_modules:",
                "    raise SystemExit(','.join(project_modules))",
            ]
        )

        result = subprocess.run(
            [sys.executable, "-c", script],
            cwd=Path(__file__).resolve().parent.parent,
            capture_output=True,
            text=True,
            check=False,
        )

        self.assertEqual(result.returncode, 0, result.stderr or result.stdout)

    def test_app_controller_constructs_app_layer_collaborators(self):
        controller = self._controller()

        self.assertIsInstance(controller._session, ProjectSession)
        self.assertIsInstance(controller._marker_editing, MarkerEditingService)
        self.assertIsInstance(controller._edit_history, EditHistory)
        self.assertIsInstance(controller._viewport, TimelineViewport)
        self.assertIsInstance(controller._waveform_lod, WaveformLodStore)

    def test_controller_exposes_undo_redo_state_and_clears_history_on_new_project(self):
        controller = self._controller()

        self.assertFalse(controller.canUndo)
        self.assertFalse(controller.canRedo)
        controller.load_demo_project()
        editable_id = self._track_by_type(controller, TrackType.EDITABLE).id
        controller.select_track(editable_id)
        marker_id = controller.add_marker_to_selected_track(0.75, "Cue", "cue", "cyan")

        self.assertTrue(controller.canUndo)
        self.assertFalse(controller.canRedo)
        self.assertEqual(controller.selectedMarkerIds, [marker_id])
        self.assertTrue(controller.isDirty)
        self.assertTrue(controller.undo())
        self.assertFalse(any(marker.id == marker_id for marker in controller._project.markers))
        self.assertEqual(controller.selectedMarkerIds, [])
        self.assertTrue(controller.canRedo)
        self.assertFalse(controller.isDirty)

        self.assertTrue(controller.redo())
        self.assertTrue(any(marker.id == marker_id for marker in controller._project.markers))
        self.assertTrue(controller.isDirty)

        controller.new_project()
        self.assertFalse(controller.canUndo)
        self.assertFalse(controller.canRedo)

    def test_undo_preserves_dirty_state_from_non_history_changes(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            source_id = controller.import_audio(str(audio_path))
            controller.add_manual_cue_track("Manual Cues")

        self.assertNotEqual(source_id, "")
        self.assertTrue(controller.isDirty)
        self.assertTrue(controller.undo())
        self.assertTrue(controller.isDirty)

    def test_undo_redo_recomputes_visible_waveform_for_current_scroll(self):
        from autolight.project.store import add_generated_track

        controller = self._controller()
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path, frames=800000)
            source_id = controller.import_audio(str(audio_path))

        waveform = add_generated_track(
            controller._project,
            parent_track_id=source_id,
            name="Waveform",
            transform_id="waveform.summary",
            transform_params={"buckets": 100},
            transform_version="1",
            output_schema="artifact.waveform.v1",
            dependency_hash="waveform-test",
        )
        waveform.result_state = ResultState.COMPLETE
        waveform.cache_refs = ["cache_waveform"]
        waveform.provenance["waveform_payload"] = {
            "version": 2,
            "duration": 100.0,
            "levels": [
                {"bucket_count": 100, "samples": [{"peak": 0.2, "rms": 0.1}] * 100}
            ],
        }
        controller._project.cache_entries.append(
            CacheEntry(
                id="cache_waveform",
                dependency_hash="dep",
                artifact_kind="waveform",
                path="waveform/cache_waveform.bin",
                created_at="",
                transform_version="1",
            )
        )
        controller.trackModel.set_project(controller._project)
        controller._refresh_visible_waveforms()
        before_project = copy.deepcopy(controller._project)
        controller._project.name = "Changed"
        controller._push_project_snapshot_command(before_project)

        def first_visible_time() -> float:
            model = controller.trackModel
            row = self._track_row_for_transform(controller, "waveform.summary")
            samples = model.data(
                model.index(row, 0),
                model.role_for_name("visibleWaveformSamples"),
            )
            return samples[0]["time"]

        self.assertEqual(first_visible_time(), 0.0)
        controller.set_timeline_scroll_seconds(50.0)
        scrolled_time = first_visible_time()
        self.assertGreater(scrolled_time, 40.0)

        self.assertTrue(controller.undo())
        self.assertAlmostEqual(first_visible_time(), scrolled_time)

        self.assertTrue(controller.redo())
        self.assertAlmostEqual(first_visible_time(), scrolled_time)

    def test_controller_loads_demo_project_into_timeline_model(self):
        controller = self._controller()

        controller.load_demo_project()

        self.assertEqual(controller.trackModel.rowCount(), 4)
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

    def test_job_queue_track_notifications_use_qt_queue_bridge(self):
        controller = self._controller()

        self.assertEqual(controller._job_queue._on_track_changed.__name__, "_queue_track_changed")

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
                {
                    "name": "Waveform Summary",
                    "trackType": "generated",
                    "resultState": "complete",
                    "markerCount": 0,
                },
            ],
        )

    def test_demo_source_track_keeps_waveform_out_of_source_cache_refs(self):
        controller = self._controller()

        controller.load_demo_project()

        source = self._track_by_type(controller, TrackType.SOURCE)
        dependency_inputs = track_dependency_inputs(controller._project, source)

        self.assertEqual(source.cache_refs, [])
        self.assertEqual(len(dependency_inputs), 1)
        self.assertTrue(dependency_inputs[0].startswith(f"track:{source.id}:"))
        self.assertNotIn("cache_", dependency_inputs[0])

    def test_controller_uses_unique_demo_audio_paths(self):
        controller = self._controller()

        controller.load_demo_project()
        first_path = controller._project.audio_assets[0].path
        controller.load_demo_project()
        second_path = controller._project.audio_assets[0].path

        self.assertNotEqual(first_path, second_path)

    def test_cleanup_unloads_playback_before_removing_demo_audio(self):
        controller = self._controller()
        controller.load_demo_project()
        demo_temp_dir = controller._demo_temp_dir
        original_demo_cleanup = demo_temp_dir.cleanup
        calls = []

        controller.playback.unload = Mock(side_effect=lambda: calls.append("unload"))
        demo_temp_dir.cleanup = Mock(
            side_effect=lambda: (calls.append("demo_cleanup"), original_demo_cleanup())
        )

        controller.cleanup()

        self.assertEqual(calls[:2], ["unload", "demo_cleanup"])

    def test_reloading_demo_unloads_playback_before_removing_previous_demo_audio(self):
        controller = self._controller()
        controller.load_demo_project()
        demo_temp_dir = controller._demo_temp_dir
        original_demo_cleanup = demo_temp_dir.cleanup
        calls = []

        controller.playback.unload = Mock(side_effect=lambda: calls.append("unload"))
        demo_temp_dir.cleanup = Mock(
            side_effect=lambda: (calls.append("demo_cleanup"), original_demo_cleanup())
        )

        controller.load_demo_project()

        self.assertEqual(calls[:2], ["unload", "demo_cleanup"])

    def test_demo_project_emits_final_timeline_duration_after_content_loads(self):
        controller = self._controller()
        duration_changes = []
        controller.timelineDurationSecondsChanged.connect(
            lambda: duration_changes.append(controller.timelineDurationSeconds)
        )

        controller.load_demo_project()

        self.assertGreater(controller.timelineDurationSeconds, 0.0)
        self.assertEqual(duration_changes[-1], controller.timelineDurationSeconds)

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

    def test_selected_source_track_can_play(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path, frames=16000)
            track_id = controller.import_audio(str(audio_path))

            self.assertEqual(controller.selectedTrackId, track_id)
            self.assertTrue(controller.selectedTrackCanPlay)
            self.assertAlmostEqual(controller.timelineDurationSeconds, 2.0, places=2)

    def test_playback_duration_change_notifies_timeline_duration(self):
        controller = self._controller()
        duration_changes = []
        controller.timelineDurationSecondsChanged.connect(
            lambda: duration_changes.append(controller.timelineDurationSeconds)
        )

        controller.playback._handle_duration_changed(2_000)

        self.assertEqual(duration_changes, [2.0])
        self.assertEqual(controller.timelineDurationSeconds, 2.0)

    def test_playback_position_change_keeps_playhead_visible_during_playback(self):
        controller = self._controller()
        controller.set_timeline_visible_seconds(4.0)
        controller.playback._handle_duration_changed(20_000)
        controller.playback._set_is_playing(True)
        scroll_changes = []
        controller.timelineScrollSecondsChanged.connect(
            lambda: scroll_changes.append(controller.timelineScrollSeconds)
        )

        controller.playback._handle_position_changed(6_500)

        self.assertEqual(len(scroll_changes), 1)
        self.assertAlmostEqual(scroll_changes[0], 3.3)
        self.assertAlmostEqual(controller.timelineScrollSeconds, 3.3)

    def test_playback_follow_updates_scroll_only_when_playhead_enters_edge_band(self):
        controller = self._controller()
        controller.set_timeline_visible_seconds(10.0)
        controller.set_timeline_scroll_seconds(20.0)

        next_scroll = controller._viewport.scroll_for_follow(
            position_seconds=25.0,
            scroll_seconds=20.0,
            visible_seconds=10.0,
            duration_seconds=60.0,
        )
        self.assertEqual(next_scroll, 20.0)

        next_scroll = controller._viewport.scroll_for_follow(
            position_seconds=29.5,
            scroll_seconds=20.0,
            visible_seconds=10.0,
            duration_seconds=60.0,
        )
        self.assertGreater(next_scroll, 20.0)
        self.assertLessEqual(next_scroll, 50.0)

    def test_playback_follow_throttles_scroll_updates(self):
        controller = self._controller()
        controller.set_timeline_visible_seconds(10.0)
        controller.set_timeline_scroll_seconds(0.0)

        self.assertTrue(controller._viewport.should_emit_follow_scroll(0.000))
        self.assertFalse(controller._viewport.should_emit_follow_scroll(0.010))
        self.assertTrue(controller._viewport.should_emit_follow_scroll(0.034))

    def test_zoom_around_anchor_keeps_anchor_screen_position_stable(self):
        controller = self._controller()
        controller.set_timeline_visible_seconds(10.0)
        controller.set_timeline_scroll_seconds(4.0)

        zoom, scroll = controller._viewport.zoom_around_anchor(
            current_zoom=100.0,
            requested_zoom=200.0,
            current_scroll=4.0,
            visible_seconds=10.0,
            duration_seconds=30.0,
            anchor_seconds=6.0,
        )

        self.assertEqual(zoom, 200.0)
        self.assertAlmostEqual(scroll, 5.0)

    def test_zoom_with_loaded_offscreen_playback_anchors_to_viewport_center(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path, frames=800000)
            controller.import_audio(str(audio_path))
            controller.playback._source_path = str(audio_path)
            controller.playback._set_position_seconds(2.0)
            controller.set_timeline_visible_seconds(10.0)
            controller.set_timeline_scroll_seconds(60.0)

            controller.set_timeline_zoom(200.0)

        self.assertEqual(controller.timelinePixelsPerSecond, 200.0)
        self.assertAlmostEqual(controller.timelineScrollSeconds, 62.6)

    def test_zoom_near_timeline_end_preserves_post_zoom_scroll_anchor(self):
        controller = self._controller()
        controller._timeline_duration_seconds = Mock(return_value=100.0)
        controller.set_timeline_visible_seconds(10.0)
        controller.set_timeline_scroll_seconds(90.0)

        controller.set_timeline_zoom(200.0)

        self.assertEqual(controller.timelinePixelsPerSecond, 200.0)
        self.assertAlmostEqual(controller.timelineScrollSeconds, 92.6)

    def test_play_selected_track_loads_resolved_source_audio(self):
        controller = self._controller()
        controller.playback.load_source = Mock(return_value=True)
        controller.playback.play = Mock()

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path, frames=12000)
            controller.import_audio(str(audio_path))

            self.assertTrue(controller.play_selected_track())

        controller.playback.load_source.assert_called_once()
        loaded_path, loaded_duration = controller.playback.load_source.call_args.args
        self.assertEqual(loaded_path, str(audio_path))
        self.assertAlmostEqual(loaded_duration, 1.5, places=2)
        controller.playback.play.assert_called_once()

    def test_play_selected_track_reuses_loaded_source_audio(self):
        controller = self._controller()
        controller.playback.load_source = Mock(return_value=True)
        controller.playback.play = Mock()

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path, frames=12000)
            controller.import_audio(str(audio_path))
            controller.playback._source_path = str(audio_path)

            self.assertTrue(controller.play_selected_track())

        controller.playback.load_source.assert_not_called()
        controller.playback.play.assert_called_once()

    def test_nudge_playback_seeks_relative_to_current_position(self):
        controller = self._controller()
        controller.playback.play = Mock()

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path, frames=80000)
            controller.import_audio(str(audio_path))
            self.assertTrue(controller.play_selected_track())

            controller.seek_playback(2.0)
            controller.nudge_playback(1.5)

            self.assertEqual(controller.playback.positionSeconds, 3.5)

    def test_play_selected_generated_track_loads_resolved_source_audio(self):
        controller = self._controller()
        controller.playback.load_source = Mock(return_value=True)
        controller.playback.play = Mock()

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path, frames=12000)
            source_id = controller.import_audio(str(audio_path))
            controller.add_fixed_interval_track(source_id, 2.0, 0.5)

            self.assertTrue(controller.play_selected_track())

        loaded_path, loaded_duration = controller.playback.load_source.call_args.args
        self.assertEqual(loaded_path, str(audio_path))
        self.assertAlmostEqual(loaded_duration, 1.5, places=2)
        controller.playback.play.assert_called_once()

    def test_play_selected_track_copies_playback_last_error_when_load_source_fails(self):
        controller = self._controller()

        class FakePlayback:
            def __init__(self):
                self.load_source = Mock(return_value=False)
                self.play = Mock()
                self.unload = Mock()
                self.lastError = "direct lastError should not be used"

            @staticmethod
            def property(name):
                if name == "sourcePath":
                    return ""
                if name == "lastError":
                    return "decoder failed"
                raise AssertionError(f"unexpected property lookup: {name}")

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path, frames=12000)
            controller.import_audio(str(audio_path))
            controller._playback = FakePlayback()

            self.assertFalse(controller.play_selected_track())

        controller.playback.load_source.assert_called_once()
        controller.playback.play.assert_not_called()
        self.assertEqual(controller.lastError, "decoder failed")

    def test_play_selected_track_rejects_track_without_source_audio(self):
        controller = self._controller()
        controller.load_demo_project()
        editable_id = controller._project.tracks[-1].id
        controller._project.audio_assets.clear()
        controller.select_track(editable_id)

        self.assertFalse(controller.play_selected_track())

        self.assertIn("source audio", controller.lastError)

    def test_timeline_zoom_and_scroll_are_clamped(self):
        controller = self._controller()
        self.assertEqual(controller.timelinePixelsPerSecond, 96.0)

        controller.set_timeline_zoom(500.0)
        self.assertEqual(controller.timelinePixelsPerSecond, 240.0)

        controller.set_timeline_zoom(5.0)
        self.assertEqual(controller.timelinePixelsPerSecond, 24.0)

        controller.set_timeline_scroll_seconds(-10.0)
        self.assertEqual(controller.timelineScrollSeconds, 0.0)

    def test_timeline_zoom_zero_and_negative_values_clamp_to_minimum(self):
        controller = self._controller()

        controller.set_timeline_zoom(120.0)
        controller.set_timeline_zoom(0.0)
        self.assertEqual(controller.timelinePixelsPerSecond, 24.0)

        controller.set_timeline_zoom(120.0)
        controller.set_timeline_zoom(-10.0)
        self.assertEqual(controller.timelinePixelsPerSecond, 24.0)

    def test_timeline_zoom_and_scroll_ignore_non_finite_values(self):
        controller = self._controller()
        controller.load_demo_project()
        controller.select_track(self._track_id(controller, 2))
        controller.add_marker_to_selected_track(20.0, "Look")
        controller.set_timeline_zoom(120.0)
        controller.set_timeline_scroll_seconds(4.0)

        controller.set_timeline_zoom(math.nan)
        controller.set_timeline_zoom(math.inf)
        controller.set_timeline_scroll_seconds(math.nan)
        controller.set_timeline_scroll_seconds(math.inf)

        self.assertEqual(controller.timelinePixelsPerSecond, 120.0)
        self.assertEqual(controller.timelineScrollSeconds, 4.0)

    def test_timeline_visible_seconds_controls_scroll_clamp(self):
        controller = self._controller()
        controller.load_demo_project()
        controller.select_track(self._track_id(controller, 2))
        controller.add_marker_to_selected_track(20.0, "Look")
        visible_changes = []
        scroll_changes = []
        controller.timelineVisibleSecondsChanged.connect(
            lambda: visible_changes.append(controller.timelineVisibleSeconds)
        )
        controller.timelineScrollSecondsChanged.connect(lambda: scroll_changes.append(controller.timelineScrollSeconds))

        controller.set_timeline_visible_seconds(2.0)
        controller.set_timeline_scroll_seconds(50.0)

        self.assertEqual(visible_changes, [2.0])
        self.assertEqual(controller.timelineScrollSeconds, 18.0)

        controller.set_timeline_visible_seconds(8.0)

        self.assertEqual(controller.timelineScrollSeconds, 12.0)
        self.assertIn(12.0, scroll_changes)

    def test_seek_playback_clamps_to_loaded_media_duration_without_inflating_transport(self):
        controller = self._controller()
        controller.load_demo_project()
        source_id = self._track_id(controller, 0)
        editable_id = self._track_id(controller, 2)
        controller.select_track(source_id)
        self.assertTrue(controller.play_selected_track())
        controller.select_track(editable_id)
        controller.add_marker_to_selected_track(20.0, "Long tail")

        controller.seek_playback(20.0)

        self.assertEqual(controller.playback.durationSeconds, 1.0)
        self.assertEqual(controller.playback.positionSeconds, 1.0)
        self.assertEqual(controller.timelineDurationSeconds, 20.0)

    def test_timeline_visible_seconds_ignores_non_finite_and_clamps_minimum(self):
        controller = self._controller()
        controller.set_timeline_visible_seconds(4.0)

        controller.set_timeline_visible_seconds(math.nan)
        controller.set_timeline_visible_seconds(math.inf)

        self.assertEqual(controller.timelineVisibleSeconds, 4.0)

        controller.set_timeline_visible_seconds(0.0)

        self.assertEqual(controller.timelineVisibleSeconds, 0.01)

    def test_qml_exposes_transport_controls_and_playhead(self):
        qml = self._qml_text(
            "UI/Main.qml",
            "UI/components/PlaybackBar.qml",
            "UI/components/TimelineLane.qml",
        )

        self.assertIn("appController.play_selected_track()", qml)
        self.assertIn("appController.pause_playback()", qml)
        self.assertIn("appController.stop_playback()", qml)
        self.assertIn("appController.seek_playback", qml)
        self.assertIn("appController.selectedTrackCanPlay", qml)
        self.assertIn("appController.playback.isPlaying", qml)
        self.assertIn("appController.playback.positionSeconds", qml)
        self.assertIn("appController.playback.durationSeconds", qml)
        self.assertIn("appController.timelinePixelsPerSecond", qml)
        self.assertIn("id: playhead", qml)
        self.assertIn("playheadTimeLabel", qml)

    def test_qml_exposes_direct_marker_drag_resize_and_manual_tracks(self):
        lane_qml = self._qml_text("UI/components/TimelineLane.qml")
        marker_qml = self._qml_text("UI/components/MarkerBlock.qml")
        toolbar_qml = self._qml_text("UI/components/ProjectToolbar.qml")
        timeline_qml = self._qml_text("UI/components/TimelineView.qml")

        self.assertIn("add_manual_cue_track", toolbar_qml)
        self.assertIn("snap_timeline_time", lane_qml)
        self.assertIn("set_timeline_visible_track_range", timeline_qml)
        self.assertIn("updateVisibleTrackRange()", timeline_qml)
        self.assertIn("move_selected_markers", marker_qml)
        self.assertIn("resize_marker", marker_qml)
        self.assertIn("AltModifier", marker_qml)

    def test_qml_marker_resize_uses_drag_delta_from_press(self):
        marker_qml = self._qml_text("UI/components/MarkerBlock.qml")

        self.assertIn("property real startParentX", marker_qml)
        self.assertIn("property real startDuration", marker_qml)
        self.assertIn("startParentX = parentX(mouse)", marker_qml)
        self.assertIn("startDuration = root.duration", marker_qml)
        self.assertIn("var widthDelta = parentX(mouse) - startParentX", marker_qml)
        self.assertIn("mapToItem(root.parent", marker_qml)
        self.assertIn("if (Math.abs(widthDelta) < root.dragThresholdPixels)", marker_qml)
        self.assertIn("startDuration + widthDelta / Math.max(1, root.pixelsPerSecond)", marker_qml)
        self.assertIn("resize_marker", marker_qml)
        self.assertNotIn("var widthDelta = mouse.x\n", marker_qml)
        self.assertNotIn("mouse.x - startX", marker_qml)
        self.assertNotIn("property real startWidth", marker_qml)
        self.assertNotIn("startWidth + widthDelta", marker_qml)

    def test_qml_marker_resize_handle_preserves_min_width_marker_selection(self):
        marker_qml = self._qml_text("UI/components/MarkerBlock.qml")

        self.assertIn("width: Math.min(8, Math.max(0, root.width / 3))", marker_qml)
        self.assertIn("visible: root.editable && root.duration > 0 && root.width > 16", marker_qml)
        self.assertIn("root.selected(root.markerId, additive)", marker_qml)
        self.assertLess(
            marker_qml.index("root.selected(root.markerId, additive)"),
            marker_qml.index("var nextDuration = Math.max"),
        )
        self.assertNotIn("visible: root.editable\n", marker_qml)

    def test_qml_marker_move_skips_click_release_below_drag_threshold(self):
        marker_qml = self._qml_text("UI/components/MarkerBlock.qml")

        self.assertIn("property real dragThresholdPixels", marker_qml)
        self.assertIn("property real pressParentX", marker_qml)
        self.assertIn("pressParentX = parentX(mouse)", marker_qml)
        self.assertIn("var pixelDelta = parentX(mouse) - pressParentX", marker_qml)
        self.assertIn("if (Math.abs(pixelDelta) < root.dragThresholdPixels)", marker_qml)
        self.assertNotIn("var pixelDelta = mouse.x - pressX", marker_qml)
        self.assertLess(
            marker_qml.index("if (Math.abs(pixelDelta) < root.dragThresholdPixels)"),
            marker_qml.index("root.appController.move_selected_markers"),
        )

    def test_qml_marker_drag_preview_affects_rendered_position_without_live_move(self):
        lane_qml = self._qml_text("UI/components/TimelineLane.qml")
        marker_qml = self._qml_text("UI/components/MarkerBlock.qml")

        self.assertIn("property real baseX", marker_qml)
        self.assertIn("property real lastPreviewDelta", marker_qml)
        self.assertIn("x: root.baseX + root.lastPreviewDelta * root.pixelsPerSecond", marker_qml)
        self.assertIn("root.lastPreviewDelta = pixelDelta / Math.max(1, root.pixelsPerSecond)", marker_qml)
        self.assertIn("root.lastPreviewDelta = 0", marker_qml)
        self.assertIn("mapToItem(root.parent", marker_qml)
        self.assertIn("baseX: root.timelineX(modelData.timestamp)", lane_qml)
        self.assertNotIn("x: root.timelineX(modelData.timestamp)", lane_qml)
        self.assertLess(
            marker_qml.index("onReleased: function(mouse)"),
            marker_qml.index("root.appController.move_selected_markers"),
        )
        self.assertLess(
            marker_qml.index("onPositionChanged: function(mouse)"),
            marker_qml.index("onReleased: function(mouse)"),
        )

    def test_qml_marker_operations_select_track_and_preserve_selected_drags(self):
        lane_qml = self._qml_text("UI/components/TimelineLane.qml")
        marker_qml = self._qml_text("UI/components/MarkerBlock.qml")

        self.assertIn("property string trackId", marker_qml)
        self.assertIn("property bool markerSelected", marker_qml)
        self.assertIn("trackId: root.trackId", lane_qml)
        self.assertIn("markerSelected: modelData.selected", lane_qml)
        self.assertIn("root.appController.select_track(root.trackId)", marker_qml)
        self.assertIn("if (!root.markerSelected)", marker_qml)
        self.assertLess(
            lane_qml.index("root.appController.select_track(root.trackId)"),
            lane_qml.index("root.appController.toggle_marker_selection"),
        )
        self.assertLess(
            marker_qml.index("root.appController.select_track(root.trackId)"),
            marker_qml.index("root.appController.move_selected_markers"),
        )

    def test_qml_project_toolbar_requires_app_controller(self):
        toolbar_qml = self._qml_text("UI/components/ProjectToolbar.qml")

        self.assertIn("required property var appController", toolbar_qml)

    def test_qml_exposes_undo_redo_actions(self):
        qml = self._qml_text("UI/Main.qml")
        toolbar_qml = self._qml_text("UI/components/ProjectToolbar.qml")

        self.assertIn("appController.undo()", toolbar_qml + qml)
        self.assertIn("appController.redo()", toolbar_qml + qml)
        self.assertIn("canUndo", toolbar_qml + qml)
        self.assertIn("canRedo", toolbar_qml + qml)

    def test_qml_main_composes_milestone_2_components(self):
        qml = self._qml_text("UI/Main.qml")

        for component_name in [
            "ProjectToolbar",
            "TransformBar",
            "PlaybackBar",
            "TimelineRuler",
            "TimelineView",
            "MarkerInspector",
            "StatusFooter",
        ]:
            with self.subTest(component_name=component_name):
                self.assertIn(component_name, qml)

    def test_qml_large_components_are_split_out_of_main(self):
        root = Path(__file__).resolve().parents[1]
        self.assertLessEqual(len((root / "UI/Main.qml").read_text(encoding="utf-8").splitlines()), 360)
        for path in [
            root / "UI/components/TimelineLane.qml",
            root / "UI/components/MarkerBlock.qml",
            root / "UI/components/WaveformStrip.qml",
        ]:
            with self.subTest(path=str(path)):
                self.assertTrue(path.exists())

    def test_qml_exposes_polished_playback_and_waveform_controls(self):
        qml = self._qml_text(
            "UI/Main.qml",
            "UI/components/PlaybackBar.qml",
            "UI/components/WaveformStrip.qml",
        )

        self.assertIn("id: playbackControls", qml)
        self.assertIn("id: playbackVolumeSlider", qml)
        self.assertIn("appController.nudge_playback", qml)
        self.assertIn("appController.playback.set_volume", qml)
        self.assertIn("root.seekTimelineAtX", qml)
        self.assertIn("Canvas", qml)
        self.assertIn("sample.rms", qml)
        self.assertIn("var centerY = height / 2", qml)

    def test_qml_waveform_uses_canvas_and_visible_samples(self):
        waveform_qml = self._qml_text("UI/components/WaveformStrip.qml")
        track_row_qml = self._qml_text("UI/components/TrackRow.qml")
        lane_qml = self._qml_text("UI/components/TimelineLane.qml")

        self.assertIn("Canvas", waveform_qml)
        self.assertIn("visibleWaveformSamples", lane_qml)
        self.assertNotIn("model: waveformSamples", waveform_qml)
        self.assertNotIn("required property var waveformSamples", track_row_qml)
        self.assertNotIn("property var waveformSamples", lane_qml)
        self.assertNotIn("waveformSamples:", track_row_qml)

    def test_qml_scrubber_avoids_live_heavy_seek_binding(self):
        playback_qml = self._qml_text("UI/components/PlaybackBar.qml")

        self.assertIn("onMoved", playback_qml)
        self.assertIn("onPressedChanged", playback_qml)
        self.assertIn("seekRequested", playback_qml)
        self.assertIn("pressedSourcePath", playback_qml)
        self.assertIn("pressedDurationSeconds", playback_qml)
        self.assertIn("onSourcePathChanged", playback_qml)
        self.assertIn("onDurationSecondsChanged", playback_qml)
        self.assertIn("root.appController.playback.sourcePath === pressedSourcePath", playback_qml)
        self.assertLess(
            playback_qml.index("root.appController.playback.sourcePath === pressedSourcePath"),
            playback_qml.index("root.seekRequested"),
        )

    def test_qml_waveform_strip_guards_invalid_samples(self):
        waveform_qml = self._qml_text("UI/components/WaveformStrip.qml")

        self.assertIn("function finiteNumber", waveform_qml)
        self.assertIn("if (!sample || typeof sample !== \"object\")", waveform_qml)
        self.assertIn("var sampleTime = root.finiteNumber(sample.time, NaN)", waveform_qml)
        self.assertIn("if (!isFinite(sampleTime))", waveform_qml)
        self.assertIn("root.clampedUnit(sample.peak)", waveform_qml)
        self.assertIn("root.clampedUnit(sample.rms)", waveform_qml)

    def test_qml_waveform_strip_repaints_render_affecting_inputs(self):
        waveform_qml = self._qml_text("UI/components/WaveformStrip.qml")

        self.assertIn("onLeftPaddingChanged: requestPaint()", waveform_qml)
        self.assertIn("onPeakColorChanged: requestPaint()", waveform_qml)
        self.assertIn("onRmsColorChanged: requestPaint()", waveform_qml)

    def test_qml_waveform_strip_batches_peak_and_rms_paths(self):
        waveform_qml = self._qml_text("UI/components/WaveformStrip.qml")

        self.assertIn("var drawableSamples = []", waveform_qml)
        self.assertIn("for (var peakIndex = 0; peakIndex < drawableSamples.length; peakIndex++)", waveform_qml)
        self.assertIn("for (var rmsIndex = 0; rmsIndex < drawableSamples.length; rmsIndex++)", waveform_qml)
        self.assertEqual(waveform_qml.count("ctx.stroke()"), 2)

    def test_qml_keeps_playback_controls_out_of_top_toolbar(self):
        toolbar_qml = self._qml_text("UI/components/ProjectToolbar.qml")
        playback_qml = self._qml_text("UI/components/PlaybackBar.qml")

        self.assertNotIn("id: playbackControls", toolbar_qml)
        self.assertNotIn("appController.nudge_playback", toolbar_qml)
        self.assertIn("id: playbackControls", playback_qml)

    def test_qml_dark_surface_action_controls_use_readable_text_color(self):
        qml = self._qml_text("UI/Main.qml")
        action_qml = self._qml_text("UI/components/TransformBar.qml")
        playback_qml = self._qml_text("UI/components/PlaybackBar.qml")

        self.assertIn('readonly property color textPrimary: "#f4f4f5"', qml)
        self.assertIn("readonly property color controlTextColor: root.textPrimary", qml)
        self.assertIn("color: root.controlTextColor", action_qml)
        self.assertIn("placeholderTextColor: root.controlMutedTextColor", action_qml)
        self.assertGreaterEqual(action_qml.count("palette.buttonText: root.controlTextColor"), 4)
        self.assertGreaterEqual(playback_qml.count("palette.buttonText: root.controlTextColor"), 4)

    def test_qml_playback_fallback_only_runs_without_selected_track(self):
        qml = self._qml_text("UI/Main.qml", "UI/components/PlaybackBar.qml")

        no_selected_track_guard = (
            "appController.selectedTrackId.length === 0 && appController.playback.sourcePath.length > 0"
        )
        root_no_selected_track_guard = (
            "root.appController.selectedTrackId.length === 0 && root.appController.playback.sourcePath.length > 0"
        )
        self.assertIn(no_selected_track_guard, qml)
        self.assertIn(
            "root.appController.selectedTrackCanPlay || (" + root_no_selected_track_guard + ") || root.appController.playback.isPlaying",
            qml,
        )
        self.assertIn("} else if (" + no_selected_track_guard + ") {", qml)
        self.assertIn("appController.playback.play()", qml)
        self.assertLess(
            qml.index("} else if (" + no_selected_track_guard + ") {"),
            qml.index("appController.playback.play()"),
        )

    def test_qml_exposes_timeline_zoom_and_scroll_controls(self):
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")

        self.assertIn("id: timelineZoomSlider", qml)
        self.assertIn("appController.set_timeline_zoom", qml)
        self.assertIn("id: timelineScrollSlider", qml)
        self.assertIn("appController.set_timeline_scroll_seconds", qml)
        self.assertIn("appController.timelinePixelsPerSecond", qml)
        self.assertIn("appController.timelineScrollSeconds", qml)
        self.assertIn("appController.timelineDurationSeconds", qml)
        self.assertIn("appController.timelineVisibleSeconds", qml)
        self.assertIn("appController.set_timeline_visible_seconds", qml)
        self.assertNotIn("readonly property real timelinePixelsPerSecond: 96", qml)
        self.assertNotIn("property real timelineVisibleSeconds: 8.0", qml)
        self.assertNotIn("root.timelineVisibleSeconds", qml)

    def test_qml_ruler_ticks_depend_on_timeline_zoom(self):
        qml = self._qml_text("UI/components/TimelineRuler.qml")

        self.assertIn("function tickStepSeconds()", qml)
        self.assertIn("timelineRuler.appController.timelinePixelsPerSecond", qml)
        self.assertIn("property real secondsPerTick: timelineRuler.tickStepSeconds()", qml)
        self.assertIn("Math.ceil(timelineRuler.appController.timelineVisibleSeconds / timelineTickLane.secondsPerTick) + 2", qml)
        self.assertIn("index * timelineTickLane.secondsPerTick", qml)
        self.assertIn("x: timelineRuler.timelineX(tickSecond)", qml)
        self.assertIn("text: timelineRuler.formatTick(tickSecond)", qml)
        self.assertNotIn("model: Math.ceil(timelineRuler.appController.timelineVisibleSeconds) + 1", qml)

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

    def test_save_and_open_restores_timeline_zoom_scroll_and_selected_track(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            project_path = root / "show.autolight"
            write_wav(audio_path, frames=120000)
            source_id = controller.import_audio(str(audio_path))
            generated_id = controller.add_fixed_interval_track(source_id, 15.0, 0.5)
            controller.select_track(generated_id)
            controller.set_timeline_visible_seconds(4.0)
            controller.seek_playback(2.25)
            controller.set_timeline_zoom(144.0)
            controller.set_timeline_scroll_seconds(3.0)

            self.assertTrue(controller.save_project(str(project_path)))
            saved = json.loads(project_path.read_text(encoding="utf-8"))

            reopened = self._controller()
            self.assertTrue(reopened.open_project(str(project_path)))

        self.assertEqual(
            saved["ui_state"]["timeline"],
            {
                "selected_track_id": generated_id,
                "pixels_per_second": 144.0,
                "scroll_seconds": 3.0,
            },
        )
        self.assertNotIn("playback", saved["ui_state"])
        self.assertEqual(reopened.selectedTrackId, generated_id)
        self.assertEqual(reopened.timelinePixelsPerSecond, 144.0)
        self.assertEqual(reopened.timelineScrollSeconds, 3.0)
        self.assertEqual(reopened.playback.positionSeconds, 0.0)

    def test_open_project_ignores_missing_selected_track_in_ui_state(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            project_path = root / "show.autolight"
            write_wav(audio_path)
            controller.import_audio(str(audio_path))
            self.assertTrue(controller.save_project(str(project_path)))
            self._write_saved_ui_state(
                project_path,
                {
                    "timeline": {
                        "selected_track_id": "missing_track",
                        "pixels_per_second": 120.0,
                        "scroll_seconds": 1.0,
                    }
                },
            )

            reopened = self._controller()
            self.assertTrue(reopened.open_project(str(project_path)))

        self.assertEqual(reopened.selectedTrackId, "")
        self.assertEqual(reopened.timelinePixelsPerSecond, 120.0)
        self.assertEqual(reopened.lastError, "")

    def test_ui_state_values_are_clamped_when_restored(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            project_path = root / "show.autolight"
            write_wav(audio_path, frames=80000)
            source_id = controller.import_audio(str(audio_path))
            self.assertTrue(controller.save_project(str(project_path)))
            self._write_saved_ui_state(
                project_path,
                {
                    "timeline": {
                        "selected_track_id": source_id,
                        "pixels_per_second": 999.0,
                        "scroll_seconds": 99.0,
                    }
                },
            )

            reopened = self._controller()
            self.assertTrue(reopened.open_project(str(project_path)))

        self.assertEqual(reopened.selectedTrackId, source_id)
        self.assertEqual(reopened.timelinePixelsPerSecond, 240.0)
        self.assertAlmostEqual(reopened.timelineVisibleSeconds, 3.2)
        self.assertAlmostEqual(reopened.timelineScrollSeconds, 6.8)

    def test_open_project_ignores_malformed_ui_state_containers(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            project_path = root / "show.autolight"
            write_wav(audio_path)
            controller.import_audio(str(audio_path))
            self.assertTrue(controller.save_project(str(project_path)))

            for ui_state in ("bad-state", ["timeline"], {"timeline": "bad-timeline"}):
                with self.subTest(ui_state=ui_state):
                    self._write_saved_ui_state(project_path, ui_state)
                    reopened = self._controller()

                    self.assertTrue(reopened.open_project(str(project_path)))
                    self.assertEqual(reopened.lastError, "")
                    self.assertEqual(reopened.selectedTrackId, "")
                    self.assertEqual(reopened.timelinePixelsPerSecond, 96.0)
                    self.assertEqual(reopened.timelineScrollSeconds, 0.0)

    def test_open_project_without_zoom_state_resets_previous_zoom(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            project_path = root / "show.autolight"
            write_wav(audio_path)
            controller.import_audio(str(audio_path))
            self.assertTrue(controller.save_project(str(project_path)))
            self._write_saved_ui_state(project_path, {})
            controller.set_timeline_zoom(180.0)

            self.assertTrue(controller.open_project(str(project_path)))

        self.assertEqual(controller.timelinePixelsPerSecond, 96.0)

    def test_open_project_ignores_non_numeric_timeline_ui_state_values(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            project_path = root / "show.autolight"
            write_wav(audio_path)
            source_id = controller.import_audio(str(audio_path))
            self.assertTrue(controller.save_project(str(project_path)))
            self._write_saved_ui_state(
                project_path,
                {
                    "timeline": {
                        "selected_track_id": source_id,
                        "pixels_per_second": "wide",
                        "scroll_seconds": {"seconds": 4.0},
                    }
                },
            )

            reopened = self._controller()
            self.assertTrue(reopened.open_project(str(project_path)))

        self.assertEqual(reopened.lastError, "")
        self.assertEqual(reopened.selectedTrackId, source_id)
        self.assertEqual(reopened.timelinePixelsPerSecond, 96.0)
        self.assertEqual(reopened.timelineScrollSeconds, 0.0)

    def test_open_project_ignores_oversized_timeline_ui_state_values(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            project_path = root / "show.autolight"
            write_wav(audio_path)
            source_id = controller.import_audio(str(audio_path))
            self.assertTrue(controller.save_project(str(project_path)))
            oversized = 10**400
            self._write_saved_ui_state(
                project_path,
                {
                    "timeline": {
                        "selected_track_id": source_id,
                        "pixels_per_second": oversized,
                        "scroll_seconds": oversized,
                    }
                },
            )

            reopened = self._controller()
            self.assertTrue(reopened.open_project(str(project_path)))

        self.assertEqual(reopened.lastError, "")
        self.assertEqual(reopened.selectedTrackId, source_id)
        self.assertEqual(reopened.timelinePixelsPerSecond, 96.0)
        self.assertEqual(reopened.timelineScrollSeconds, 0.0)

    def test_open_project_clears_marker_selection_when_restoring_same_track(self):
        controller = self._controller()
        controller.load_demo_project()
        editable_id = self._track_id(controller, 2)
        controller.select_track(editable_id)
        marker_id = self._selected_track_markers(controller)[0]["id"]
        controller.toggle_marker_selection(marker_id, False)

        with tempfile.TemporaryDirectory() as tmp:
            project_path = Path(tmp) / "show.autolight"
            self.assertTrue(controller.save_project(str(project_path)))
            self.assertEqual(controller.selectedTrackId, editable_id)
            self.assertEqual(controller.selectedMarkerIds, [marker_id])

            self.assertTrue(controller.open_project(str(project_path)))

        self.assertEqual(controller.lastError, "")
        self.assertEqual(controller.selectedTrackId, editable_id)
        self.assertEqual(controller.selectedMarkerIds, [])

    def test_marker_span_selection_follows_controller_selection_state(self):
        controller = self._controller()
        controller.load_demo_project()
        editable_id = self._track_id(controller, 2)
        controller.select_track(editable_id)
        marker_id = controller.add_marker_to_selected_track(1.25, "Hit")
        controller.clear_marker_selection()

        def marker_span_selected() -> bool:
            model = controller.trackModel
            spans = model.data(model.index(2, 0), model.role_for_name("markerSpans"))
            for span in spans:
                if span["id"] == marker_id:
                    return span["selected"]
            self.fail(f"marker span not found: {marker_id}")

        self.assertFalse(marker_span_selected())

        controller.toggle_marker_selection(marker_id, False)

        self.assertTrue(marker_span_selected())
        marker = self._marker_by_id(controller, marker_id)
        self.assertNotIn("selected", marker.metadata)

        controller.toggle_marker_selection(marker_id, True)

        self.assertFalse(marker_span_selected())

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

    def test_open_project_marks_offline_audio_tracks_stale(self):
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
            self.assertTrue(controller.save_project(str(project_path)))
            audio_path.unlink()

            reopened = self._controller()
            self.assertTrue(reopened.open_project(str(project_path)))

        reopened_source = self._track_by_id(reopened, source_id)
        reopened_generated = self._track_by_id(reopened, generated_id)
        source_index = reopened.trackModel.index(0, 0)
        result_role = reopened.trackModel.role_for_name("resultState")
        error_role = reopened.trackModel.role_for_name("error")
        self.assertEqual(reopened_source.result_state, ResultState.STALE)
        self.assertIn("audio asset offline", reopened_source.error)
        self.assertEqual(reopened_generated.result_state, ResultState.STALE)
        self.assertEqual(reopened.trackModel.data(source_index, result_role), "stale")
        self.assertIn("audio asset offline", reopened.trackModel.data(source_index, error_role))

    def test_open_project_restores_source_track_when_missing_audio_returns(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            write_wav(audio_path)
            audio_payload = audio_path.read_bytes()
            project_path = root / "show.autolight"
            source_id = controller.import_audio(str(audio_path))
            self.assertTrue(controller.save_project(str(project_path)))
            audio_path.unlink()

            offline = self._controller()
            self.assertTrue(offline.open_project(str(project_path)))
            self.assertTrue(offline.save_project(str(project_path)))
            audio_path.write_bytes(audio_payload)

            restored = self._controller()
            self.assertTrue(restored.open_project(str(project_path)))

        restored_source = self._track_by_id(restored, source_id)
        self.assertEqual(restored._project.audio_assets[0].import_status, "online")
        self.assertEqual(restored_source.result_state, ResultState.COMPLETE)
        self.assertEqual(restored_source.error, "")

    def test_open_project_restores_audio_staled_dependents_when_audio_returns(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            write_wav(audio_path)
            audio_payload = audio_path.read_bytes()
            project_path = root / "show.autolight"
            source_id = controller.import_audio(str(audio_path))
            generated_id = controller.add_fixed_interval_track(source_id, 2.0, 0.5)
            generated = self._track_by_id(controller, generated_id)
            generated.result_state = ResultState.COMPLETE
            self.assertTrue(controller.save_project(str(project_path)))
            audio_path.unlink()

            offline = self._controller()
            self.assertTrue(offline.open_project(str(project_path)))
            offline_generated = self._track_by_id(offline, generated_id)
            self.assertEqual(offline_generated.result_state, ResultState.STALE)
            self.assertTrue(offline.save_project(str(project_path)))
            audio_path.write_bytes(audio_payload)

            restored = self._controller()
            self.assertTrue(restored.open_project(str(project_path)))

        restored_source = self._track_by_id(restored, source_id)
        restored_generated = self._track_by_id(restored, generated_id)
        self.assertEqual(restored_source.result_state, ResultState.COMPLETE)
        self.assertEqual(restored_generated.result_state, ResultState.COMPLETE)
        self.assertEqual(restored_generated.error, "")

    def test_open_project_marks_modified_audio_tracks_stale(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            write_wav(audio_path, frames=8000)
            project_path = root / "show.autolight"
            source_id = controller.import_audio(str(audio_path))
            self.assertTrue(controller.save_project(str(project_path)))
            write_wav(audio_path, frames=4000)

            reopened = self._controller()
            self.assertTrue(reopened.open_project(str(project_path)))

        reopened_source = self._track_by_id(reopened, source_id)
        self.assertEqual(reopened._project.audio_assets[0].import_status, "modified")
        self.assertEqual(reopened_source.result_state, ResultState.STALE)
        self.assertIn("audio asset modified", reopened_source.error)

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

    def test_rerun_track_rejects_stale_input_tracks(self):
        controller = self._controller()
        controller.load_demo_project()
        source = self._track_by_id(controller, self._track_id(controller, 0))
        generated_id = self._track_id(controller, 1)
        generated = self._track_by_id(controller, generated_id)
        source.result_state = ResultState.STALE
        source.error = "audio asset offline: song.wav"
        generated.result_state = ResultState.STALE
        controller.select_track(generated_id)

        job_id = controller.rerun_track(generated_id)

        self.assertEqual(job_id, "")
        self.assertFalse(controller.selectedTrackCanRerun)
        self.assertEqual(generated.result_state, ResultState.STALE)
        self.assertIn("input track is not complete", controller.lastError)

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

    def test_rerun_track_recomputes_dependency_hash_from_editable_markers(self):
        from autolight.project.store import add_generated_track

        controller = self._controller()
        controller.load_demo_project()
        editable = self._track_by_id(controller, self._track_id(controller, 2))
        child = add_generated_track(
            controller._project,
            editable.id,
            "Derived",
            "markers.fixed_interval",
            {"duration": 1.0, "interval": 0.5},
            "1",
            "markers.v1",
            "old_dependency_hash",
        )

        first_job_id = controller.rerun_track(child.id)
        controller._job_queue.wait(first_job_id, timeout=2)
        first_dependency_hash = child.dependency_hash
        controller.select_track(editable.id)
        controller.add_marker_to_selected_track(1.75, "Blackout")
        child.result_state = ResultState.STALE

        second_job_id = controller.rerun_track(child.id)
        controller._job_queue.wait(second_job_id, timeout=2)

        self.assertNotEqual(first_job_id, "")
        self.assertNotEqual(second_job_id, "")
        self.assertNotEqual(child.dependency_hash, first_dependency_hash)

    def test_add_marker_emits_timeline_duration_and_reclamps_scroll(self):
        controller = self._controller()
        controller.load_demo_project()
        editable_id = self._track_id(controller, 2)
        controller.select_track(editable_id)
        controller._timeline_scroll_seconds = 50.0
        duration_changes = []
        scroll_changes = []
        controller.timelineDurationSecondsChanged.connect(
            lambda: duration_changes.append(controller.timelineDurationSeconds)
        )
        controller.timelineScrollSecondsChanged.connect(lambda: scroll_changes.append(controller.timelineScrollSeconds))

        marker_id = controller.add_marker_to_selected_track(12.0, "Look")

        self.assertNotEqual(marker_id, "")
        self.assertEqual(duration_changes, [12.0])
        self.assertEqual(scroll_changes, [4.0])
        self.assertEqual(controller.timelineScrollSeconds, 4.0)

    def test_delete_marker_emits_timeline_duration_and_reclamps_scroll(self):
        controller = self._controller()
        controller.load_demo_project()
        editable_id = self._track_id(controller, 2)
        controller.select_track(editable_id)
        marker_id = controller.add_marker_to_selected_track(20.0, "Look")
        controller.set_timeline_scroll_seconds(12.0)
        duration_changes = []
        scroll_changes = []
        controller.timelineDurationSecondsChanged.connect(
            lambda: duration_changes.append(controller.timelineDurationSeconds)
        )
        controller.timelineScrollSecondsChanged.connect(lambda: scroll_changes.append(controller.timelineScrollSeconds))

        self.assertTrue(controller.delete_marker_from_selected_track(marker_id))

        self.assertEqual(duration_changes, [1.0])
        self.assertEqual(scroll_changes, [0.0])
        self.assertEqual(controller.timelineScrollSeconds, 0.0)

    def test_handle_track_changed_reclamps_timeline_scroll_after_duration_change(self):
        controller = self._controller()
        controller.load_demo_project()
        controller._timeline_scroll_seconds = 50.0
        scroll_changes = []
        controller.timelineScrollSecondsChanged.connect(lambda: scroll_changes.append(controller.timelineScrollSeconds))

        controller._handle_track_changed(controller.selectedTrackId)

        self.assertEqual(scroll_changes, [0.0])
        self.assertEqual(controller.timelineScrollSeconds, 0.0)

    def test_non_waveform_track_change_does_not_refresh_waveform_rows(self):
        controller = self._controller()
        controller.load_demo_project()
        waveform_row = self._track_row_for_transform(controller, "waveform.summary")
        source_id = self._track_id(controller, 0)
        emissions = []
        controller.trackModel.dataChanged.connect(
            lambda top_left, _bottom_right, _roles: emissions.append(top_left.row())
        )

        controller._handle_track_changed(source_id)

        self.assertNotIn(waveform_row, emissions)

    def test_selected_track_markers_changed_emits_when_selected_job_updates_markers(self):
        controller = self._controller()
        controller.load_demo_project()
        generated_id = self._track_id(controller, 1)
        controller.select_track(generated_id)
        marker_changes = []
        controller.selectedTrackMarkersChanged.connect(lambda: marker_changes.append(self._selected_track_markers(controller)))

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
        self.assertEqual(controller.trackModel.rowCount(), 5)
        self.assertEqual(controller.selectedTrackId, editable_id)
        editable = self._track_by_id(controller, editable_id)
        self.assertEqual(editable.input_track_ids, [generated_id])
        self.assertEqual(editable.result_state, ResultState.COMPLETE)

    def test_controller_adds_manual_cue_track_from_selected_source_context(self):
        controller = self._controller()
        controller.load_demo_project()
        source = self._track_by_type(controller, TrackType.SOURCE)
        controller.select_track(source.id)

        manual_id = controller.add_manual_cue_track("Manual Cues")

        self.assertNotEqual(manual_id, "")
        manual = self._track_by_id(controller, manual_id)
        self.assertEqual(manual.type, TrackType.EDITABLE)
        self.assertEqual(manual.input_track_ids, [source.id])
        self.assertEqual(controller.selectedTrackId, manual_id)
        self.assertTrue(controller.canUndo)

    def test_manual_track_undo_preserves_later_imported_audio_track(self):
        controller = self._controller()
        controller.load_demo_project()
        source = self._track_by_type(controller, TrackType.SOURCE)
        controller.select_track(source.id)
        manual_id = controller.add_manual_cue_track("Manual Cues")

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "later.wav"
            write_wav(audio_path)
            imported_id = controller.import_audio(str(audio_path))

        self.assertNotEqual(manual_id, "")
        self.assertNotEqual(imported_id, "")
        self.assertTrue(controller.undo())
        self.assertIsNone(self._optional_track_by_id(controller, manual_id))
        self.assertIsNotNone(self._optional_track_by_id(controller, imported_id))

        self.assertTrue(controller.redo())
        self.assertIsNotNone(self._optional_track_by_id(controller, manual_id))
        self.assertIsNotNone(self._optional_track_by_id(controller, imported_id))

    def test_obsolete_manual_track_undo_reports_no_reverted_edit(self):
        from autolight.project.store import add_generated_track

        controller = self._controller()
        controller.load_demo_project()
        source = self._track_by_type(controller, TrackType.SOURCE)
        controller.select_track(source.id)
        manual_id = controller.add_manual_cue_track("Manual Cues")
        dependent = add_generated_track(
            controller._project,
            manual_id,
            "Generated From Manual",
            "markers.fixed_interval",
            {},
            "1",
            "markers.v1",
            "dep",
        )

        self.assertFalse(controller.undo())

        self.assertIsNotNone(self._optional_track_by_id(controller, manual_id))
        self.assertIsNotNone(self._optional_track_by_id(controller, dependent.id))
        self.assertFalse(controller.canUndo)
        self.assertFalse(controller.canRedo)

    def test_controller_exposes_default_qml_slot_overloads_for_task5_slots(self):
        controller = self._controller()

        self.assertIn((), self._slot_parameter_types(controller, "add_manual_cue_track"))
        self.assertIn(("QString",), self._slot_parameter_types(controller, "add_manual_cue_track"))
        self.assertIn(("double",), self._slot_parameter_types(controller, "move_selected_markers"))
        self.assertIn(("double", "bool"), self._slot_parameter_types(controller, "move_selected_markers"))
        self.assertIn(("double",), self._slot_parameter_types(controller, "snap_timeline_time"))
        self.assertIn(("double", "bool"), self._slot_parameter_types(controller, "snap_timeline_time"))
        self.assertIn(
            ("double", "double", "QString", "QString", "QString"),
            self._slot_parameter_types(controller, "add_marker_to_selected_track_with_duration"),
        )
        self.assertIn(
            ("double", "double", "QString", "QString", "QString"),
            self._slot_parameter_types(controller, "update_selected_marker_with_duration"),
        )
        self.assertIn((), self._slot_parameter_types(controller, "delete_selected_markers"))

    def test_controller_moves_and_resizes_selected_editable_markers(self):
        controller = self._controller()
        controller.load_demo_project()
        editable = self._track_by_type(controller, TrackType.EDITABLE)
        controller.select_track(editable.id)
        marker_id = controller.add_marker_to_selected_track(0.5, "Cue", "cue", "cyan")
        controller.toggle_marker_selection(marker_id, False)

        self.assertTrue(controller.move_selected_markers(0.25, False))
        marker = self._marker_by_id(controller, marker_id)
        self.assertEqual(marker.timestamp, 0.75)

        self.assertTrue(controller.resize_marker(marker_id, 1.25))
        self.assertEqual(marker.duration, 1.25)

    def test_controller_rejects_non_finite_marker_move_deltas_before_snapping(self):
        for delta in [math.nan, math.inf, -math.inf]:
            with self.subTest(delta=delta):
                controller = self._controller()
                controller.load_demo_project()
                editable = self._track_by_type(controller, TrackType.EDITABLE)
                controller.select_track(editable.id)
                marker_id = controller.add_marker_to_selected_track(0.5, "Cue", "cue", "cyan")
                controller.toggle_marker_selection(marker_id, False)
                marker = self._marker_by_id(controller, marker_id)
                before_dirty = controller.isDirty
                before_can_undo = controller.canUndo
                before_can_redo = controller.canRedo
                before_undo_count = len(controller._edit_history._undo_stack)
                before_redo_count = len(controller._edit_history._redo_stack)

                self.assertFalse(controller.move_selected_markers(delta, False))

                self.assertEqual(marker.timestamp, 0.5)
                self.assertEqual(controller.isDirty, before_dirty)
                self.assertEqual(controller.canUndo, before_can_undo)
                self.assertEqual(controller.canRedo, before_can_redo)
                self.assertEqual(len(controller._edit_history._undo_stack), before_undo_count)
                self.assertEqual(len(controller._edit_history._redo_stack), before_redo_count)
                self.assertIn("finite", controller.lastError)

    def test_controller_snap_time_uses_generated_timing_markers_and_bypass(self):
        controller = self._controller()
        controller.load_demo_project()
        timing = self._track_by_name(controller, "Beat Markers")

        self.assertEqual(controller.snap_timeline_time(0.53, False), 0.5)
        self.assertEqual(controller.snap_timeline_time(0.53, True), 0.53)
        controller.select_track(timing.id)
        self.assertEqual(controller.snap_timeline_time(1.03, False), 1.0)

    def test_controller_snap_time_uses_only_visible_timing_tracks(self):
        controller = self._controller()
        controller.load_demo_project()
        timing = self._track_by_name(controller, "Beat Markers")
        editable = self._track_by_type(controller, TrackType.EDITABLE)
        timing_row = controller._project.tracks.index(timing)
        editable_row = controller._project.tracks.index(editable)

        controller.set_timeline_visible_track_range(editable_row, 1)
        self.assertEqual(controller.snap_timeline_time(0.53, False), 0.53)

        controller.set_timeline_visible_track_range(timing_row, 1)
        self.assertEqual(controller.snap_timeline_time(0.53, False), 0.5)
        self.assertIn(("int", "int"), self._slot_parameter_types(controller, "set_timeline_visible_track_range"))

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

    def test_screenshot_argument_requires_a_non_flag_value(self):
        self.assertEqual(
            app_entry._argument_value(["main.py", "--screenshot", "capture.png"], "--screenshot"),
            "capture.png",
        )
        self.assertEqual(
            app_entry._argument_value(["main.py", "--screenshot", "--smoke"], "--screenshot"),
            "",
        )

    def test_screenshot_checker_distinguishes_blank_dark_from_toolbar_clip(self):
        from scripts import check_qml_screenshot

        blank = QImage(1120, 720, QImage.Format.Format_RGB32)
        blank.fill(QColor("#181a1f"))

        clipped = QImage(1120, 720, QImage.Format.Format_RGB32)
        clipped.fill(QColor("#181a1f"))
        for y in range(0, 40):
            for x in range(0, 1120):
                clipped.setPixelColor(x, y, QColor("#f4f4f5"))
        for y in range(0, 40, 2):
            clipped.setPixelColor(1118, y, QColor("#111318"))

        self.assertTrue(check_qml_screenshot.toolbar_right_edge_is_clear(blank))
        self.assertFalse(check_qml_screenshot.toolbar_right_edge_is_clear(clipped))

    def test_qml_timeline_shell_uses_one_row_oriented_list(self):
        qml = self._qml_text(
            "UI/components/TimelineView.qml",
            "UI/components/TimelineLane.qml",
            "UI/components/MarkerBlock.qml",
        )

        self.assertIn("id: timelineRows", qml)
        self.assertIn("model: markerSpans", qml)
        self.assertIn("marker: modelData", qml)
        self.assertIn("root.marker.timestamp", qml)
        self.assertIn("root.timelineX(root.marker.timestamp)", qml)
        self.assertIn("appController.timelinePixelsPerSecond", qml)
        self.assertIn("root.marker.duration : 0.08) * root.appController.timelinePixelsPerSecond", qml)
        self.assertIn("pixelsPerSecond: root.appController.timelinePixelsPerSecond", qml)
        self.assertIn("editable: root.editable", qml)
        self.assertNotIn("Math.max(0, Math.min(parent.width - width, root.timelineX(root.marker.timestamp)))", qml)
        self.assertNotIn("root.timelinePixelsPerSecond", qml)
        self.assertNotIn("spacing: 48", qml)
        self.assertNotIn("model: markerCount", qml)
        self.assertIn("onContentYChanged: timelineRows.updateVisibleTrackRange()", qml)
        self.assertNotIn("contentY =", qml)

    def test_qml_marker_color_options_are_bound_from_controller(self):
        controller = self._controller()
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")

        self.assertEqual(
            controller.markerColorOptions,
            [
                {"key": "cyan", "label": "Cyan", "color": "#67e8f9"},
                {"key": "green", "label": "Green", "color": "#a7f3d0"},
                {"key": "amber", "label": "Amber", "color": "#fbbf24"},
                {"key": "violet", "label": "Violet", "color": "#c4b5fd"},
                {"key": "rose", "label": "Rose", "color": "#fda4af"},
                {"key": "blue", "label": "Blue", "color": "#93c5fd"},
            ],
        )
        self.assertIn("readonly property var markerColorOptions: appController.markerColorOptions", qml)
        self.assertNotIn('{ key: "cyan", label: "Cyan", color: "#67e8f9" }', qml)

    def test_qml_uses_named_timeline_label_width(self):
        qml = self._qml_text(
            "UI/Main.qml",
            "UI/components/TimelineRuler.qml",
            "UI/components/TrackRow.qml",
        )

        self.assertIn("readonly property real timelineLabelWidth: 280", qml)
        self.assertIn("timelineView.rowsWidth - root.timelineLabelWidth - root.timelineLeftPadding", qml)
        self.assertIn("Layout.preferredWidth: timelineRuler.timelineLabelWidth", qml)
        self.assertIn("width: root.timelineLabelWidth", qml)
        self.assertIn("parent.width - root.timelineLabelWidth", qml)
        self.assertNotIn("timelineRows.width - 280 - root.timelineLeftPadding", qml)
        self.assertNotIn("Layout.preferredWidth: 280", qml)
        self.assertNotIn("width: 280", qml)

    def test_qml_timeline_ruler_has_fixed_height(self):
        qml = self._qml_text("UI/Main.qml", "UI/components/TimelineRuler.qml")

        self.assertIn("readonly property real timelineRulerHeight: 32", qml)
        self.assertIn("id: timelineRuler", qml)
        self.assertIn("Layout.minimumHeight: timelineRuler.timelineRulerHeight", qml)
        self.assertIn("Layout.preferredHeight: timelineRuler.timelineRulerHeight", qml)
        self.assertIn("Layout.maximumHeight: timelineRuler.timelineRulerHeight", qml)

    def test_qml_uses_grouped_toolbar_and_stable_lane_dimensions(self):
        qml = self._qml_text(
            "UI/Main.qml",
            "UI/components/ProjectToolbar.qml",
            "UI/components/TransformBar.qml",
            "UI/components/TimelineView.qml",
            "UI/components/TrackRow.qml",
            "UI/components/TimelineLane.qml",
        )
        toolbar_qml = self._qml_text("UI/components/ProjectToolbar.qml")
        toolbar_start = toolbar_qml.index("ToolBar {")
        toolbar_shell = toolbar_qml[toolbar_start : toolbar_qml.index("RowLayout {", toolbar_start)]

        self.assertIn("id: fileActions", qml)
        self.assertIn("id: transformActions", qml)
        self.assertIn("id: timelineControls", qml)
        self.assertIn("readonly property int timelineRowHeight", qml)
        self.assertIn("height: root.timelineRowHeight", qml)
        self.assertIn("elide: Text.ElideRight", qml)
        self.assertIn("clip: true", qml)
        self.assertNotIn("background:", toolbar_shell)

    def test_qml_exposes_project_workflow_actions(self):
        qml = self._qml_text(
            "UI/Main.qml",
            "UI/components/ProjectToolbar.qml",
            "UI/components/TransformBar.qml",
        )

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
        self.assertIn("appController.playback.lastError", qml)
        self.assertIn("statusError", qml)

    def test_qml_exposes_job_progress_controls(self):
        qml = self._qml_text(
            "UI/Main.qml",
            "UI/components/TransformBar.qml",
            "UI/components/TrackRow.qml",
        )

        self.assertIn("jobProgress", qml)
        self.assertIn("activeJobId", qml)
        self.assertIn("ProgressBar", qml)
        self.assertIn("appController.cancel_selected_job()", qml)
        self.assertIn("appController.rerun_track(appController.selectedTrackId)", qml)
        self.assertIn("enabled: root.appController.selectedTrackHasRunningJob", qml)
        self.assertIn(
            "enabled: root.appController.selectedTrackCanRerun && !root.appController.selectedTrackHasRunningJob",
            qml,
        )

    def test_qml_exposes_cache_refresh_and_rerun_recovery(self):
        qml = self._qml_text(
            "UI/Main.qml",
            "UI/components/TransformBar.qml",
            "UI/components/TrackRow.qml",
        )

        self.assertIn("appController.refresh_cache_status()", qml)
        self.assertIn("appController.rerun_track(appController.selectedTrackId)", qml)
        self.assertIn('resultState === "stale"', qml)
        self.assertIn('resultState === "failed"', qml)
        self.assertIn("text: root.error", qml)
        self.assertIn("visible: root.error.length > 0", qml)

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

    @staticmethod
    def _optional_track_by_id(controller: AppController, track_id: str):
        for track in controller._project.tracks:
            if track.id == track_id:
                return track
        return None

    def _track_by_type(self, controller: AppController, track_type: TrackType):
        for track in controller._project.tracks:
            if track.type == track_type:
                return track
        self.fail(f"track type not found: {track_type}")

    def _track_by_name(self, controller: AppController, name: str):
        for track in controller._project.tracks:
            if track.name == name:
                return track
        self.fail(f"track name not found: {name}")

    def _track_row_for_transform(self, controller: AppController, transform_id: str) -> int:
        for index, track in enumerate(controller._project.tracks):
            if track.transform_id == transform_id:
                return index
        self.fail(f"track transform not found: {transform_id}")

    def _marker_by_id(self, controller: AppController, marker_id: str):
        for marker in controller._project.markers:
            if marker.id == marker_id:
                return marker
        self.fail(f"marker not found: {marker_id}")

    @staticmethod
    def _slot_parameter_types(controller: AppController, slot_name: str) -> set[tuple[str, ...]]:
        meta_object = controller.metaObject()
        slots = set()
        for index in range(meta_object.methodCount()):
            method = meta_object.method(index)
            name = bytes(method.name()).decode()
            if name == slot_name:
                slots.add(tuple(bytes(type_name).decode() for type_name in method.parameterTypes()))
        return slots

    @staticmethod
    def _selected_track_markers(controller: AppController) -> list[dict]:
        return list(controller.selectedTrackMarkers)

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

    @staticmethod
    def _write_saved_ui_state(project_path: Path, ui_state: dict) -> None:
        payload = json.loads(project_path.read_text(encoding="utf-8"))
        payload["ui_state"] = ui_state
        project_path.write_text(json.dumps(payload, indent=2, sort_keys=True), encoding="utf-8")

    def _controller(self):
        controller = AppController()
        self.addCleanup(controller.cleanup)
        return controller


if __name__ == "__main__":
    unittest.main()
