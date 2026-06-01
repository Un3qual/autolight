import math
import tempfile
import unittest
from pathlib import Path

from autolight.project.models import Marker, ResultState
from autolight.project.store import (
    MARKER_COLOR_PALETTE,
    add_editable_marker,
    add_generated_track,
    bulk_update_editable_markers,
    create_editable_track_from_markers,
    delete_editable_marker,
    import_audio_asset,
    marker_display_color,
    new_project,
    update_editable_marker,
)
from tests.helpers import write_wav


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

    def test_add_editable_marker_marks_downstream_generated_tracks_stale(self):
        project = new_project("Demo")
        editable = self._editable_track(project)
        downstream = add_generated_track(
            project,
            editable.id,
            "Generated From Editable",
            "markers.fixed_interval",
            {},
            "1",
            "markers.v1",
            "dep",
        )
        downstream.result_state = ResultState.COMPLETE

        add_editable_marker(project, editable.id, 1.25, "Cue")

        self.assertEqual(downstream.result_state, ResultState.STALE)

    def test_delete_editable_marker_marks_downstream_generated_tracks_stale(self):
        project = new_project("Demo")
        editable = self._editable_track(project)
        marker = add_editable_marker(project, editable.id, 1.25, "Cue")
        downstream = add_generated_track(
            project,
            editable.id,
            "Generated From Editable",
            "markers.fixed_interval",
            {},
            "1",
            "markers.v1",
            "dep",
        )
        downstream.result_state = ResultState.COMPLETE

        self.assertTrue(delete_editable_marker(project, editable.id, marker.id))

        self.assertEqual(downstream.result_state, ResultState.STALE)

    def test_add_editable_marker_rejects_non_finite_timestamp(self):
        project = new_project("Demo")
        generated = self._generated_track(project)
        project.markers.append(Marker(id="marker_source", track_id=generated.id, timestamp=0.5))
        editable = create_editable_track_from_markers(project, generated.id, "Editable", ["marker_source"])

        with self.assertRaisesRegex(ValueError, "finite"):
            add_editable_marker(project, editable.id, math.nan, "Cue")

        with self.assertRaisesRegex(ValueError, "finite"):
            add_editable_marker(project, editable.id, math.inf, "Cue")

    def test_update_editable_marker_sets_label_category_color_and_timestamp(self):
        project = new_project("Demo")
        editable = self._editable_track(project)
        marker = add_editable_marker(project, editable.id, 1.25, "Cue")

        updated = update_editable_marker(
            project,
            editable.id,
            marker.id,
            timestamp=2.5,
            label="Blackout",
            category="lighting",
            color="amber",
        )

        self.assertIs(updated, marker)
        self.assertEqual(marker.timestamp, 2.5)
        self.assertEqual(marker.label, "Blackout")
        self.assertEqual(marker.category, "lighting")
        self.assertEqual(marker.metadata["color"], "amber")
        self.assertEqual(marker_display_color(marker), MARKER_COLOR_PALETTE["amber"])

    def test_update_editable_marker_rejects_generated_track_and_invalid_color(self):
        project = new_project("Demo")
        generated = self._generated_track(project)
        project.markers.append(Marker(id="marker_source", track_id=generated.id, timestamp=0.5))

        with self.assertRaisesRegex(ValueError, "editable track"):
            update_editable_marker(
                project,
                generated.id,
                "marker_source",
                timestamp=1.0,
                label="Cue",
                category="cue",
                color="cyan",
            )

        editable = create_editable_track_from_markers(project, generated.id, "Editable", ["marker_source"])
        marker = [item for item in project.markers if item.track_id == editable.id][0]

        with self.assertRaisesRegex(ValueError, "marker color"):
            update_editable_marker(
                project,
                editable.id,
                marker.id,
                timestamp=1.0,
                label="Cue",
                category="cue",
                color="not-a-color",
            )

    def test_update_editable_marker_invalid_color_leaves_marker_and_downstream_unchanged(self):
        project = new_project("Demo")
        editable = self._editable_track(project)
        marker = add_editable_marker(project, editable.id, 1.25, "Cue")
        downstream = add_generated_track(
            project,
            editable.id,
            "Generated From Editable",
            "markers.fixed_interval",
            {},
            "1",
            "markers.v1",
            "dep",
        )
        downstream.result_state = ResultState.COMPLETE

        with self.assertRaisesRegex(ValueError, "marker color"):
            update_editable_marker(
                project,
                editable.id,
                marker.id,
                timestamp=2.0,
                label="Changed",
                category="changed",
                color="not-a-color",
            )

        self.assertEqual(marker.timestamp, 1.25)
        self.assertEqual(marker.label, "Cue")
        self.assertEqual(marker.category, "cue")
        self.assertEqual(marker.metadata["color"], "cyan")
        self.assertEqual(downstream.result_state, ResultState.COMPLETE)

    def test_update_editable_marker_marks_downstream_generated_tracks_stale(self):
        project = new_project("Demo")
        editable = self._editable_track(project)
        marker = add_editable_marker(project, editable.id, 1.25, "Cue")
        downstream = add_generated_track(
            project,
            editable.id,
            "Generated From Editable",
            "markers.fixed_interval",
            {},
            "1",
            "markers.v1",
            "dep",
        )
        downstream.result_state = ResultState.COMPLETE

        update_editable_marker(
            project,
            editable.id,
            marker.id,
            timestamp=1.5,
            label="Look",
            category="lighting",
            color="violet",
        )

        self.assertEqual(downstream.result_state, ResultState.STALE)

    def test_bulk_update_editable_markers_updates_named_markers(self):
        project = new_project("Demo")
        editable = self._editable_track(project)
        first = add_editable_marker(project, editable.id, 1.0, "A")
        second = add_editable_marker(project, editable.id, 2.0, "B")
        third = add_editable_marker(project, editable.id, 3.0, "C")

        updated_count = bulk_update_editable_markers(
            project,
            editable.id,
            [first.id, third.id],
            label="Hit",
            category="accent",
            color="rose",
        )

        self.assertEqual(updated_count, 2)
        self.assertEqual(first.label, "Hit")
        self.assertEqual(first.category, "accent")
        self.assertEqual(first.metadata["color"], "rose")
        self.assertEqual(second.label, "B")
        self.assertEqual(second.metadata["color"], "cyan")
        self.assertEqual(third.label, "Hit")

    def test_bulk_update_with_empty_marker_ids_updates_all_markers_on_track(self):
        project = new_project("Demo")
        generated = self._generated_track(project)
        editable = create_editable_track_from_markers(project, generated.id, "Editable", [])
        first = add_editable_marker(project, editable.id, 1.0, "A")
        second = add_editable_marker(project, editable.id, 2.0, "B")

        updated_count = bulk_update_editable_markers(
            project,
            editable.id,
            [],
            label="Scene",
            category="scene",
            color="blue",
        )

        self.assertEqual(updated_count, 2)
        self.assertEqual([first.label, second.label], ["Scene", "Scene"])
        self.assertEqual([first.metadata["color"], second.metadata["color"]], ["blue", "blue"])

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

    def test_controller_invalid_add_marker_color_is_atomic(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        downstream = add_generated_track(
            controller._project,
            editable_id,
            "Generated From Editable",
            "markers.fixed_interval",
            {},
            "1",
            "markers.v1",
            "dep",
        )
        downstream.result_state = ResultState.COMPLETE
        before_marker_ids = [
            marker.id for marker in controller._project.markers if marker.track_id == editable_id
        ]
        controller._set_dirty(False)

        marker_id = controller.add_marker_to_selected_track(1.5, "Broken", "cue", "not-a-color")

        after_marker_ids = [
            marker.id for marker in controller._project.markers if marker.track_id == editable_id
        ]
        self.assertEqual(marker_id, "")
        self.assertIn("marker color", controller.lastError)
        self.assertEqual(after_marker_ids, before_marker_ids)
        self.assertFalse(controller.isDirty)
        self.assertEqual(downstream.result_state, ResultState.COMPLETE)

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

    def test_controller_tracks_selected_marker_ids(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        first_marker_id = controller.selectedTrackMarkers[0]["id"]
        second_marker_id = controller.selectedTrackMarkers[1]["id"]

        controller.toggle_marker_selection(first_marker_id, False)
        controller.toggle_marker_selection(second_marker_id, True)

        self.assertEqual(controller.selectedMarkerIds, [first_marker_id, second_marker_id])
        first_marker = controller.selectedTrackMarkers[0]
        second_marker = controller.selectedTrackMarkers[1]
        self.assertEqual(
            set(first_marker),
            {"id", "timestamp", "label", "category", "color", "colorKey", "selected"},
        )
        self.assertEqual(first_marker["color"], MARKER_COLOR_PALETTE["cyan"])
        self.assertEqual(first_marker["colorKey"], "cyan")
        self.assertTrue(first_marker["selected"])
        self.assertTrue(second_marker["selected"])

    def test_controller_clears_marker_selection_when_selected_track_changes(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        generated_id = self._track_id_for_type(controller, "generated")
        controller.select_track(editable_id)
        marker_id = controller.selectedTrackMarkers[0]["id"]
        controller.toggle_marker_selection(marker_id, False)

        controller.select_track(generated_id)

        self.assertEqual(controller.selectedMarkerIds, [])
        self.assertFalse(any(marker["selected"] for marker in controller.selectedTrackMarkers))

    def test_controller_track_change_with_selected_marker_emits_marker_summary_once(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        generated_id = self._track_id_for_type(controller, "generated")
        controller.select_track(editable_id)
        marker_id = controller.selectedTrackMarkers[0]["id"]
        controller.toggle_marker_selection(marker_id, False)
        marker_changes = []
        controller.selectedTrackMarkersChanged.connect(lambda: marker_changes.append(controller.selectedTrackMarkers))

        controller.select_track(generated_id)

        self.assertEqual(len(marker_changes), 1)
        self.assertEqual(controller.selectedMarkerIds, [])
        self.assertFalse(any(marker["selected"] for marker in controller.selectedTrackMarkers))

    def test_controller_update_selected_marker_changes_marker_fields(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        marker_id = controller.selectedTrackMarkers[0]["id"]
        controller.toggle_marker_selection(marker_id, False)

        self.assertTrue(controller.update_selected_marker(1.75, "Blackout", "lighting", "amber"))

        marker = next(item for item in controller._project.markers if item.id == marker_id)
        self.assertEqual(marker.timestamp, 1.75)
        self.assertEqual(marker.label, "Blackout")
        self.assertEqual(marker.category, "lighting")
        self.assertEqual(marker.metadata["color"], "amber")
        self.assertEqual(controller.selectedMarkerIds, [marker_id])
        self.assertEqual(controller.timelineDurationSeconds, 1.75)
        self.assertEqual(controller.lastError, "")
        self.assertTrue(controller.isDirty)

    def test_controller_noop_update_selected_marker_does_not_dirty_or_refresh(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        marker_id = controller.add_marker_to_selected_track(1.5, "Blackout")
        controller.toggle_marker_selection(marker_id, False)
        controller._set_dirty(False)
        marker_changes = []
        duration_changes = []
        model_resets = []
        controller.selectedTrackMarkersChanged.connect(lambda: marker_changes.append(controller.selectedTrackMarkers))
        controller.timelineDurationSecondsChanged.connect(lambda: duration_changes.append(controller.timelineDurationSeconds))
        controller.trackModel.modelReset.connect(lambda: model_resets.append(True))

        self.assertTrue(controller.update_selected_marker(1.5, "Blackout", "cue", "cyan"))

        self.assertFalse(controller.isDirty)
        self.assertEqual(controller.lastError, "")
        self.assertEqual(marker_changes, [])
        self.assertEqual(duration_changes, [])
        self.assertEqual(model_resets, [])

    def test_controller_bulk_update_selected_markers_updates_selected_or_all(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        first_marker_id = controller.selectedTrackMarkers[0]["id"]
        second_marker_id = controller.selectedTrackMarkers[1]["id"]
        controller.toggle_marker_selection(first_marker_id, False)

        self.assertEqual(controller.bulk_update_selected_markers("Scene", "scene", "violet"), 1)
        first = next(item for item in controller._project.markers if item.id == first_marker_id)
        second = next(item for item in controller._project.markers if item.id == second_marker_id)
        self.assertEqual(first.label, "Scene")
        self.assertNotEqual(second.label, "Scene")
        self.assertEqual(controller.lastError, "")
        self.assertTrue(controller.isDirty)

        controller.clear_marker_selection()
        self.assertEqual(controller.bulk_update_selected_markers("All", "scene", "blue"), 2)
        self.assertEqual([item["label"] for item in controller.selectedTrackMarkers], ["All", "All"])
        self.assertEqual([item["colorKey"] for item in controller.selectedTrackMarkers], ["blue", "blue"])

    def test_controller_noop_bulk_update_selected_markers_does_not_dirty_or_refresh(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        first_marker_id = controller.selectedTrackMarkers[0]["id"]
        controller.toggle_marker_selection(first_marker_id, False)
        self.assertEqual(controller.bulk_update_selected_markers("Scene", "scene", "violet"), 1)
        controller._set_dirty(False)
        marker_changes = []
        model_resets = []
        controller.selectedTrackMarkersChanged.connect(lambda: marker_changes.append(controller.selectedTrackMarkers))
        controller.trackModel.modelReset.connect(lambda: model_resets.append(True))

        self.assertEqual(controller.bulk_update_selected_markers("Scene", "scene", "violet"), 0)

        self.assertFalse(controller.isDirty)
        self.assertEqual(controller.lastError, "")
        self.assertEqual(marker_changes, [])
        self.assertEqual(model_resets, [])

    def test_controller_delete_selected_marker_emits_marker_summary_once(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        marker_id = controller.add_marker_to_selected_track(1.5, "Blackout")
        controller.toggle_marker_selection(marker_id, False)
        marker_changes = []
        controller.selectedTrackMarkersChanged.connect(lambda: marker_changes.append(controller.selectedTrackMarkers))

        self.assertTrue(controller.delete_marker_from_selected_track(marker_id))

        self.assertEqual(len(marker_changes), 1)
        self.assertEqual(controller.selectedMarkerIds, [])

    def test_marker_summary_normalizes_invalid_color_key_to_default(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        first_marker_id = controller.selectedTrackMarkers[0]["id"]
        second_marker_id = controller.selectedTrackMarkers[1]["id"]
        first = next(item for item in controller._project.markers if item.id == first_marker_id)
        second = next(item for item in controller._project.markers if item.id == second_marker_id)
        first.metadata["color"] = "not-a-color"
        second.metadata["color"] = 42

        summaries = controller.selectedTrackMarkers

        self.assertEqual([item["color"] for item in summaries], [MARKER_COLOR_PALETTE["cyan"]] * 2)
        self.assertEqual([item["colorKey"] for item in summaries], ["cyan", "cyan"])

    def test_qml_exposes_editable_marker_inspector(self):
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")

        self.assertIn("id: inspectorPanel", qml)
        self.assertIn("markerTimestampField", qml)
        self.assertIn("markerLabelField", qml)
        self.assertIn("appController.selectedTrackMarkers", qml)
        self.assertIn("inspectorPanel.selectedMarkerId", qml)
        self.assertIn("appController.add_marker_to_selected_track", qml)
        self.assertIn("appController.delete_marker_from_selected_track(inspectorPanel.selectedMarkerId)", qml)
        self.assertIn("appController.selectedTrackIsEditable", qml)
        self.assertIn(
            "enabled: appController.selectedTrackId.length > 0 && appController.selectedTrackIsEditable",
            qml,
        )
        self.assertIn(
            "enabled: inspectorPanel.selectedMarkerId.length > 0 && appController.selectedTrackIsEditable",
            qml,
        )

    def test_qml_exposes_marker_label_color_and_bulk_edit_controls(self):
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")

        self.assertIn("id: markerColorPicker", qml)
        self.assertIn("id: markerCategoryField", qml)
        self.assertIn("appController.toggle_marker_selection", qml)
        self.assertIn("appController.update_selected_marker", qml)
        self.assertIn("appController.bulk_update_selected_markers", qml)
        self.assertIn("modelData.color", qml)
        self.assertIn("modelData.selected", qml)
        self.assertIn("selectedMarkerIds.length", qml)
        self.assertIn("function syncMarkerEditorFromSelection()", qml)
        self.assertIn("root.syncMarkerEditorFromSelection()", qml)
        self.assertNotIn("root.syncMarkerEditor(modelData)", qml)

    @staticmethod
    def _generated_track(project):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            source = import_audio_asset(project, audio_path)

        return add_generated_track(project, source.id, "Generated", "markers.fixed_interval", {}, "1", "markers.v1", "hash")

    @staticmethod
    def _editable_track(project):
        generated = EditableMarkerInspectorTest._generated_track(project)
        project.markers.append(Marker(id="marker_source", track_id=generated.id, timestamp=0.5))
        return create_editable_track_from_markers(project, generated.id, "Editable", ["marker_source"])

    @staticmethod
    def _track_id_for_type(controller, track_type: str) -> str:
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
