import copy
import json
import math
import tempfile
import unittest
from pathlib import Path

from autolight.app.edit_history import (
    EditHistory,
    MarkerSnapshotCommand,
    ProjectSnapshotCommand,
    TrackSnapshotCommand,
)
from autolight.app.marker_editing import MarkerEditingService
from autolight.project.models import JobRun, Marker, ResultState, TrackType
from autolight.project.store import (
    MARKER_COLOR_PALETTE,
    ProjectStore,
    add_editable_marker,
    add_generated_track,
    bulk_update_editable_markers,
    create_manual_editable_track,
    create_editable_track_from_markers,
    delete_editable_marker,
    import_audio_asset,
    marker_color_key,
    marker_display_color,
    marker_snapshot,
    move_editable_markers,
    new_project,
    resize_editable_marker,
    update_editable_marker,
)
from tests.helpers import write_wav


class EditableMarkerInspectorTest(unittest.TestCase):
    def test_create_manual_editable_track_uses_resolved_source_track(self):
        project = new_project("Demo")
        source = self._source_track(project)
        generated = add_generated_track(
            project,
            source.id,
            "Generated",
            "markers.fixed_interval",
            {},
            "1",
            "markers.v1",
            "dep",
        )

        manual = create_manual_editable_track(project, generated.id, "Manual Cues")

        self.assertEqual(manual.type, TrackType.EDITABLE)
        self.assertEqual(manual.input_track_ids, [source.id])
        self.assertEqual(manual.result_state, ResultState.COMPLETE)
        self.assertEqual(manual.provenance["manual_track"], True)
        self.assertEqual(manual.provenance["created_by"], "user")

    def test_create_manual_editable_track_rejects_track_without_source_context(self):
        project = new_project("Demo")

        with self.assertRaisesRegex(ValueError, "source audio"):
            create_manual_editable_track(project, "", "Manual Cues")

    def test_move_editable_markers_is_atomic_for_negative_result(self):
        project = new_project("Demo")
        editable = self._editable_track(project)
        first = add_editable_marker(project, editable.id, 0.25, "First")
        second = add_editable_marker(project, editable.id, 1.25, "Second")

        with self.assertRaisesRegex(ValueError, "negative timestamp"):
            move_editable_markers(project, editable.id, [first.id, second.id], -0.5)

        self.assertEqual(first.timestamp, 0.25)
        self.assertEqual(second.timestamp, 1.25)

    def test_move_editable_markers_rejects_non_finite_results_atomically(self):
        project = new_project("Demo")
        editable = self._editable_track(project)
        marker = add_editable_marker(project, editable.id, 1e308, "Huge")

        with self.assertRaisesRegex(ValueError, "finite"):
            move_editable_markers(project, editable.id, [marker.id], 1e308)

        self.assertEqual(marker.timestamp, 1e308)

        marker.timestamp = math.inf
        with self.assertRaisesRegex(ValueError, "finite"):
            move_editable_markers(project, editable.id, [marker.id], 0.0)
        self.assertTrue(math.isinf(marker.timestamp))

    def test_move_editable_markers_noop_does_not_mark_downstream_stale(self):
        project = new_project("Demo")
        editable = self._editable_track(project)
        marker = add_editable_marker(project, editable.id, 0.25, "Cue")
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

        moved = move_editable_markers(project, editable.id, [marker.id], 0.0)

        self.assertEqual(moved, [marker])
        self.assertEqual(marker.timestamp, 0.25)
        self.assertEqual(downstream.result_state, ResultState.COMPLETE)

    def test_resize_editable_marker_sets_duration_and_rejects_negative_duration(self):
        project = new_project("Demo")
        editable = self._editable_track(project)
        marker = add_editable_marker(project, editable.id, 0.25, "Cue")

        resize_editable_marker(project, editable.id, marker.id, 1.5)
        self.assertEqual(marker.duration, 1.5)

        with self.assertRaisesRegex(ValueError, "duration"):
            resize_editable_marker(project, editable.id, marker.id, -0.1)
        self.assertEqual(marker.duration, 1.5)

    def test_marker_editing_service_snaps_to_visible_timing_markers(self):
        project = new_project("Demo")
        source = self._source_track(project)
        timing = add_generated_track(
            project,
            source.id,
            "Beat Markers",
            "timing.beats",
            {},
            "1",
            "markers.v1",
            "dep",
        )
        timing.result_state = ResultState.COMPLETE
        project.markers.append(Marker(id="beat_1", track_id=timing.id, timestamp=1.0, category="beat"))
        service = MarkerEditingService()

        snapped = service.snap_time(
            project,
            requested_seconds=1.03,
            pixels_per_second=100.0,
            visible_track_ids=[timing.id],
            bypass=False,
        )

        self.assertEqual(snapped, 1.0)
        timing_onsets = add_generated_track(
            project,
            source.id,
            "Onset Markers",
            "timing.onsets",
            {},
            "1",
            "markers.v1",
            "dep-onsets",
        )
        timing_onsets.result_state = ResultState.COMPLETE
        project.markers.append(Marker(id="onset_1", track_id=timing_onsets.id, timestamp=1.5, category="onset"))
        self.assertEqual(
            service.snap_time(
                project,
                requested_seconds=1.53,
                pixels_per_second=100.0,
                visible_track_ids=[timing_onsets.id],
                bypass=False,
            ),
            1.5,
        )
        self.assertEqual(
            service.snap_time(
                project,
                requested_seconds=1.03,
                pixels_per_second=100.0,
                visible_track_ids=[timing.id],
                bypass=True,
            ),
            1.03,
        )

    def test_marker_editing_service_ignores_non_timing_generated_markers(self):
        project = new_project("Demo")
        source = self._source_track(project)
        generated = add_generated_track(
            project,
            source.id,
            "Fixed Markers",
            "markers.fixed_interval",
            {},
            "1",
            "markers.v1",
            "dep",
        )
        generated.result_state = ResultState.COMPLETE
        project.markers.append(Marker(id="cue_1", track_id=generated.id, timestamp=0.5, category="cue"))
        service = MarkerEditingService()

        snapped = service.snap_time(
            project,
            requested_seconds=0.53,
            pixels_per_second=100.0,
            visible_track_ids=[generated.id],
            bypass=False,
        )

        self.assertEqual(snapped, 0.53)

    def test_marker_editing_service_snap_edge_cases(self):
        project = new_project("Demo")
        source = self._source_track(project)
        stale = add_generated_track(project, source.id, "Stale Beats", "timing.beats", {}, "1", "markers.v1", "dep")
        stale.result_state = ResultState.STALE
        project.markers.append(Marker(id="stale_beat", track_id=stale.id, timestamp=1.0, category="timing"))
        service = MarkerEditingService()

        self.assertEqual(
            service.snap_time(
                project,
                requested_seconds=1.25,
                pixels_per_second=32.0,
                visible_track_ids=[stale.id],
                bypass=False,
            ),
            1.0,
        )

        failed = add_generated_track(project, source.id, "Failed Beats", "timing.beats", {}, "1", "markers.v1", "dep")
        failed.result_state = ResultState.FAILED
        running = add_generated_track(project, source.id, "Running Beats", "timing.beats", {}, "1", "markers.v1", "dep")
        running.result_state = ResultState.RUNNING
        editable = create_manual_editable_track(project, source.id, "Manual Cues")
        project.markers.extend(
            [
                Marker(id="source_marker", track_id=source.id, timestamp=2.0, category="timing"),
                Marker(id="failed_marker", track_id=failed.id, timestamp=2.0, category="timing"),
                Marker(id="running_marker", track_id=running.id, timestamp=2.0, category="timing"),
                Marker(id="editable_marker", track_id=editable.id, timestamp=2.0, category="timing"),
            ]
        )

        self.assertEqual(
            service.snap_time(
                project,
                requested_seconds=2.03,
                pixels_per_second=100.0,
                visible_track_ids=[source.id, failed.id, running.id, editable.id],
                bypass=False,
            ),
            2.03,
        )

    def test_marker_editing_service_ignores_non_finite_snap_times(self):
        project = new_project("Demo")
        source = self._source_track(project)
        timing = add_generated_track(project, source.id, "Beat Markers", "timing.beats", {}, "1", "markers.v1", "dep")
        timing.result_state = ResultState.COMPLETE
        project.markers.extend(
            [
                Marker(id="bad_nan", track_id=timing.id, timestamp=math.nan, category="timing"),
                Marker(id="bad_inf", track_id=timing.id, timestamp=math.inf, category="timing"),
            ]
        )
        service = MarkerEditingService()

        snapped = service.snap_time(
            project,
            requested_seconds=math.nan,
            pixels_per_second=100.0,
            visible_track_ids=[timing.id],
            bypass=False,
        )

        self.assertTrue(math.isfinite(snapped))
        self.assertEqual(snapped, 0.0)

    def test_marker_snapshot_deep_copies_mutable_marker_fields(self):
        marker = Marker(
            id="marker_1",
            track_id="track_1",
            timestamp=1.0,
            tags=["flash"],
            source_marker_ids=["source_1"],
            metadata={"nested": {"color": "cyan"}, "steps": ["a"]},
        )

        snapshot = marker_snapshot(marker)
        marker.tags.append("blackout")
        marker.source_marker_ids.append("source_2")
        marker.metadata["nested"]["color"] = "amber"
        marker.metadata["steps"].append("b")

        self.assertEqual(snapshot["tags"], ["flash"])
        self.assertEqual(snapshot["source_marker_ids"], ["source_1"])
        self.assertEqual(snapshot["metadata"], {"nested": {"color": "cyan"}, "steps": ["a"]})

    def test_edit_history_undoes_and_redoes_marker_snapshot_command(self):
        project = new_project("Demo")
        editable = self._editable_track(project)
        marker = add_editable_marker(project, editable.id, 1.0, "Cue", color="cyan")
        before = [marker_snapshot(marker)]
        update_editable_marker(
            project,
            editable.id,
            marker.id,
            timestamp=2.0,
            label="Hit",
            category="accent",
            color="amber",
        )
        after = [marker_snapshot(marker)]
        history = EditHistory()
        history.push(MarkerSnapshotCommand(track_id=editable.id, before=before, after=after))

        self.assertTrue(history.can_undo)
        history.undo(project)
        marker = self._project_marker_by_id(project, marker.id)
        self.assertEqual(marker.timestamp, 1.0)
        self.assertEqual(marker.label, "Cue")
        self.assertEqual(marker.metadata["color"], "cyan")

        self.assertTrue(history.can_redo)
        history.redo(project)
        marker = self._project_marker_by_id(project, marker.id)
        self.assertEqual(marker.timestamp, 2.0)
        self.assertEqual(marker.label, "Hit")
        self.assertEqual(marker.metadata["color"], "amber")

    def test_marker_snapshot_command_restores_dependent_track_state(self):
        project = new_project("Demo")
        editable = self._editable_track(project)
        marker = add_editable_marker(project, editable.id, 1.0, "Cue", color="cyan")
        downstream = add_generated_track(
            project,
            editable.id,
            "Generated From Editable",
            "markers.fixed_interval",
            {"duration": 1.0, "interval": 0.5},
            "1",
            "markers.v1",
            "complete-hash",
        )
        downstream.result_state = ResultState.COMPLETE
        downstream_marker = Marker(id="downstream_marker", track_id=downstream.id, timestamp=3.0)
        downstream_job = JobRun(
            id="downstream_job",
            track_id=downstream.id,
            transform_id=downstream.transform_id,
            parameters_hash=downstream.dependency_hash,
            state=ResultState.COMPLETE,
        )
        project.markers.append(downstream_marker)
        project.job_runs.append(downstream_job)
        before = [marker_snapshot(marker)]
        before_dependents = [
            {
                "index": project.tracks.index(downstream),
                "track": copy.deepcopy(downstream),
                "markers": [copy.deepcopy(downstream_marker)],
                "job_runs": [copy.deepcopy(downstream_job)],
            }
        ]
        update_editable_marker(
            project,
            editable.id,
            marker.id,
            timestamp=2.0,
            label="Hit",
            category="accent",
            color="amber",
        )
        after = [marker_snapshot(marker)]
        after_dependents = [
            {
                "index": project.tracks.index(downstream),
                "track": copy.deepcopy(downstream),
                "markers": [
                    copy.deepcopy(item)
                    for item in project.markers
                    if item.track_id == downstream.id
                ],
                "job_runs": [
                    copy.deepcopy(item)
                    for item in project.job_runs
                    if item.track_id == downstream.id
                ],
            }
        ]
        history = EditHistory()
        history.push(
            MarkerSnapshotCommand(
                track_id=editable.id,
                before=before,
                after=after,
                before_dependents=before_dependents,
                after_dependents=after_dependents,
            )
        )

        history.undo(project)
        restored_downstream = self._project_track_by_id(project, downstream.id)
        self.assertEqual(restored_downstream.result_state, ResultState.COMPLETE)
        self.assertEqual(restored_downstream.dependency_hash, "complete-hash")
        self.assertIn(downstream_marker.id, [item.id for item in project.markers])
        self.assertIn(downstream_job.id, [item.id for item in project.job_runs])

        history.redo(project)
        restored_downstream = self._project_track_by_id(project, downstream.id)
        self.assertEqual(restored_downstream.result_state, ResultState.STALE)

    def test_edit_history_keeps_command_on_failed_undo(self):
        class FailingCommand:
            @staticmethod
            def undo(project):
                raise RuntimeError("restore failed")

            @staticmethod
            def redo(project):
                raise AssertionError("redo should not be called")

        history = EditHistory()
        history.push(FailingCommand())

        with self.assertRaisesRegex(RuntimeError, "restore failed"):
            history.undo(new_project("Demo"))

        self.assertTrue(history.can_undo)
        self.assertFalse(history.can_redo)

    def test_edit_history_discards_track_creation_undo_when_dependents_exist(self):
        project = new_project("Demo")
        source = self._source_track(project)
        manual = create_manual_editable_track(project, source.id, "Manual Cues")
        command = TrackSnapshotCommand(
            track_id=manual.id,
            before=None,
            after=manual,
            index=project.tracks.index(manual),
        )
        history = EditHistory()
        history.push(command)
        dependent = add_generated_track(
            project,
            manual.id,
            "Generated From Manual",
            "markers.fixed_interval",
            {},
            "1",
            "markers.v1",
            "dep",
        )

        self.assertFalse(history.undo(project))

        self.assertIn(manual.id, [track.id for track in project.tracks])
        self.assertIn(dependent.id, [track.id for track in project.tracks])
        self.assertFalse(history.can_undo)
        self.assertFalse(history.can_redo)

    def test_edit_history_discards_track_deletion_redo_when_dependents_exist(self):
        project = new_project("Demo")
        source = self._source_track(project)
        manual = create_manual_editable_track(project, source.id, "Manual Cues")
        command = TrackSnapshotCommand(
            track_id=manual.id,
            before=manual,
            after=None,
            index=project.tracks.index(manual),
        )
        command.redo(project)
        history = EditHistory()
        history.push(command)
        self.assertTrue(history.undo(project))
        dependent = add_generated_track(
            project,
            manual.id,
            "Generated From Manual",
            "markers.fixed_interval",
            {},
            "1",
            "markers.v1",
            "dep",
        )

        self.assertFalse(history.redo(project))

        self.assertIn(manual.id, [track.id for track in project.tracks])
        self.assertIn(dependent.id, [track.id for track in project.tracks])
        self.assertFalse(history.can_undo)
        self.assertFalse(history.can_redo)

    def test_project_snapshot_command_restores_ui_state_in_place(self):
        project = new_project("Demo")
        project.ui_state["timeline"] = {"scroll_seconds": 4.0}
        before = new_project("Before")
        before.ui_state["timeline"] = {"scroll_seconds": 1.0}
        command = ProjectSnapshotCommand(before=before, after=project)
        ui_state = project.ui_state

        command.undo(project)

        self.assertIs(project.ui_state, ui_state)
        self.assertEqual(project.ui_state, {"timeline": {"scroll_seconds": 1.0}})

    def test_track_snapshot_command_removes_only_created_track(self):
        project = new_project("Demo")
        source = self._source_track(project)
        manual = create_manual_editable_track(project, source.id, "Manual Cues")
        marker = add_editable_marker(project, manual.id, 1.25, "Cue", duration=0.5)
        job_run = JobRun(
            id="job_manual",
            track_id=manual.id,
            transform_id="manual",
            parameters_hash="hash",
            state=ResultState.COMPLETE,
        )
        project.job_runs.append(job_run)
        command = TrackSnapshotCommand(
            track_id=manual.id,
            before=None,
            after=manual,
            index=project.tracks.index(manual),
            after_markers=[marker],
            after_job_runs=[job_run],
        )
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "later.wav"
            write_wav(audio_path)
            imported = import_audio_asset(project, audio_path)

        command.undo(project)

        self.assertNotIn(manual.id, [track.id for track in project.tracks])
        self.assertNotIn(marker.id, [item.id for item in project.markers])
        self.assertNotIn(job_run.id, [item.id for item in project.job_runs])
        self.assertIn(imported.id, [track.id for track in project.tracks])

        command.redo(project)
        self.assertIn(manual.id, [track.id for track in project.tracks])
        self.assertIn(marker.id, [item.id for item in project.markers])
        self.assertIn(job_run.id, [item.id for item in project.job_runs])
        self.assertIn(imported.id, [track.id for track in project.tracks])

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

    def test_add_editable_marker_accepts_metadata_fields(self):
        project = new_project("Demo")
        editable = self._editable_track(project)

        marker = add_editable_marker(project, editable.id, 1.25, "Cue", category="lighting", color="amber")

        self.assertEqual(marker.category, "lighting")
        self.assertEqual(marker.metadata["color"], "amber")

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

    def test_marker_display_color_tolerates_non_dict_metadata(self):
        marker = Marker(id="marker_1", track_id="track_1", timestamp=0.0)
        marker.metadata = "blue"

        self.assertEqual(marker_color_key(marker), "cyan")
        self.assertEqual(marker_display_color(marker), MARKER_COLOR_PALETTE["cyan"])

    def test_project_load_normalizes_non_dict_marker_metadata(self):
        project = new_project("Demo")
        editable = self._editable_track(project)
        marker = add_editable_marker(project, editable.id, 1.25, "Cue", color="blue")

        with tempfile.TemporaryDirectory() as tmp:
            project_path = Path(tmp) / "demo.autolight"
            ProjectStore.save(project, project_path)
            raw = json.loads(project_path.read_text(encoding="utf-8"))
            for marker_payload in raw["markers"]:
                if marker_payload["id"] == marker.id:
                    marker_payload["metadata"] = "blue"
            project_path.write_text(json.dumps(raw), encoding="utf-8")

            loaded = ProjectStore.load(project_path)

        loaded_marker = self._project_marker_by_id(loaded, marker.id)
        self.assertEqual(loaded_marker.metadata, {})
        self.assertEqual(marker_display_color(loaded_marker), MARKER_COLOR_PALETTE["cyan"])

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

    def test_controller_undo_redo_deletes_marker_history(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        marker_id = self._selected_track_markers(controller)[0]["id"]

        self.assertTrue(controller.delete_marker_from_selected_track(marker_id))
        self.assertFalse(any(marker.id == marker_id for marker in controller._project.markers))
        self.assertTrue(controller.canUndo)

        self.assertTrue(controller.undo())
        self.assertTrue(any(marker.id == marker_id for marker in controller._project.markers))
        self.assertTrue(controller.canRedo)

        self.assertTrue(controller.redo())
        self.assertFalse(any(marker.id == marker_id for marker in controller._project.markers))

    def test_controller_tracks_selected_marker_ids(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        first_marker_id = self._selected_track_markers(controller)[0]["id"]
        second_marker_id = self._selected_track_markers(controller)[1]["id"]

        controller.toggle_marker_selection(first_marker_id, False)
        controller.toggle_marker_selection(second_marker_id, True)

        self.assertEqual(controller.selectedMarkerIds, [first_marker_id, second_marker_id])
        first_marker = self._selected_track_markers(controller)[0]
        second_marker = self._selected_track_markers(controller)[1]
        self.assertEqual(
            set(first_marker),
            {"id", "timestamp", "duration", "label", "category", "color", "colorKey", "selected"},
        )
        self.assertEqual(first_marker["color"], MARKER_COLOR_PALETTE["cyan"])
        self.assertEqual(first_marker["colorKey"], "cyan")
        self.assertTrue(first_marker["selected"])
        self.assertTrue(second_marker["selected"])

    def test_controller_selected_track_marker_summary_includes_duration(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)

        marker_id = controller.add_marker_to_selected_track_with_duration(
            1.5,
            1.25,
            "Blackout",
            "cue",
            "cyan",
        )

        summary = self._selected_marker_summary_by_id(controller, marker_id)
        self.assertEqual(summary["duration"], 1.25)

    def test_controller_clears_marker_selection_when_selected_track_changes(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        generated_id = self._track_id_for_type(controller, "generated")
        controller.select_track(editable_id)
        marker_id = self._selected_track_markers(controller)[0]["id"]
        controller.toggle_marker_selection(marker_id, False)

        controller.select_track(generated_id)

        self.assertEqual(controller.selectedMarkerIds, [])
        self.assertFalse(any(marker["selected"] for marker in self._selected_track_markers(controller)))

    def test_controller_track_change_with_selected_marker_emits_marker_summary_once(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        generated_id = self._track_id_for_type(controller, "generated")
        controller.select_track(editable_id)
        marker_id = self._selected_track_markers(controller)[0]["id"]
        controller.toggle_marker_selection(marker_id, False)
        marker_changes = []
        controller.selectedTrackMarkersChanged.connect(lambda: marker_changes.append(self._selected_track_markers(controller)))

        controller.select_track(generated_id)

        self.assertEqual(len(marker_changes), 1)
        self.assertEqual(controller.selectedMarkerIds, [])
        self.assertFalse(any(marker["selected"] for marker in self._selected_track_markers(controller)))

    def test_controller_update_selected_marker_changes_marker_fields(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        marker_id = self._selected_track_markers(controller)[0]["id"]
        controller.toggle_marker_selection(marker_id, False)

        self.assertTrue(controller.update_selected_marker(1.75, "Blackout", "lighting", "amber"))

        marker = self._marker_by_id(controller, marker_id)
        self.assertEqual(marker.timestamp, 1.75)
        self.assertEqual(marker.label, "Blackout")
        self.assertEqual(marker.category, "lighting")
        self.assertEqual(marker.metadata["color"], "amber")
        self.assertEqual(controller.selectedMarkerIds, [marker_id])
        self.assertEqual(controller.timelineDurationSeconds, 1.75)
        self.assertEqual(controller.lastError, "")
        self.assertTrue(controller.isDirty)

    def test_controller_update_selected_marker_with_duration_changes_duration(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        marker_id = self._selected_track_markers(controller)[0]["id"]
        controller.toggle_marker_selection(marker_id, False)

        self.assertTrue(
            controller.update_selected_marker_with_duration(
                1.75,
                0.5,
                "Blackout",
                "lighting",
                "amber",
            )
        )

        marker = self._marker_by_id(controller, marker_id)
        self.assertEqual(marker.timestamp, 1.75)
        self.assertEqual(marker.duration, 0.5)
        self.assertEqual(marker.label, "Blackout")
        self.assertEqual(marker.category, "lighting")
        self.assertEqual(marker.metadata["color"], "amber")

    def test_controller_undo_redo_updates_selected_marker_history(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        marker_id = self._selected_track_markers(controller)[0]["id"]
        marker = self._marker_by_id(controller, marker_id)
        before = (marker.timestamp, marker.label, marker.category, marker_color_key(marker))
        controller.toggle_marker_selection(marker_id, False)

        self.assertTrue(controller.update_selected_marker(1.75, "Blackout", "lighting", "amber"))
        self.assertTrue(controller.canUndo)

        self.assertTrue(controller.undo())
        marker = self._marker_by_id(controller, marker_id)
        self.assertEqual((marker.timestamp, marker.label, marker.category, marker_color_key(marker)), before)
        self.assertTrue(controller.canRedo)

        self.assertTrue(controller.redo())
        marker = self._marker_by_id(controller, marker_id)
        self.assertEqual(marker.timestamp, 1.75)
        self.assertEqual(marker.label, "Blackout")
        self.assertEqual(marker.category, "lighting")
        self.assertEqual(marker.metadata["color"], "amber")

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
        controller.selectedTrackMarkersChanged.connect(lambda: marker_changes.append(self._selected_track_markers(controller)))
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
        first_marker_id = self._selected_track_markers(controller)[0]["id"]
        second_marker_id = self._selected_track_markers(controller)[1]["id"]
        controller.toggle_marker_selection(first_marker_id, False)

        self.assertEqual(controller.bulk_update_selected_markers("Scene", "scene", "violet"), 1)
        first = self._marker_by_id(controller, first_marker_id)
        second = self._marker_by_id(controller, second_marker_id)
        self.assertEqual(first.label, "Scene")
        self.assertNotEqual(second.label, "Scene")
        self.assertEqual(controller.lastError, "")
        self.assertTrue(controller.isDirty)

        controller.clear_marker_selection()
        self.assertEqual(controller.bulk_update_selected_markers("All", "scene", "blue"), 2)
        summaries = self._selected_track_markers(controller)
        self.assertEqual(len(summaries), 2)
        self.assertEqual(summaries[0]["label"], "All")
        self.assertEqual(summaries[1]["label"], "All")
        self.assertEqual(summaries[0]["colorKey"], "blue")
        self.assertEqual(summaries[1]["colorKey"], "blue")

    def test_controller_undo_redo_bulk_updates_selected_markers_history(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        marker_ids = [marker["id"] for marker in self._selected_track_markers(controller)]
        before = {
            marker_id: (
                self._marker_by_id(controller, marker_id).label,
                self._marker_by_id(controller, marker_id).category,
                marker_color_key(self._marker_by_id(controller, marker_id)),
            )
            for marker_id in marker_ids
        }
        controller.clear_marker_selection()

        self.assertEqual(controller.bulk_update_selected_markers("All", "scene", "blue"), 2)
        self.assertTrue(controller.canUndo)

        self.assertTrue(controller.undo())
        for marker_id, expected in before.items():
            marker = self._marker_by_id(controller, marker_id)
            self.assertEqual((marker.label, marker.category, marker_color_key(marker)), expected)
        self.assertTrue(controller.canRedo)

        self.assertTrue(controller.redo())
        for marker_id in marker_ids:
            marker = self._marker_by_id(controller, marker_id)
            self.assertEqual(marker.label, "All")
            self.assertEqual(marker.category, "scene")
            self.assertEqual(marker.metadata["color"], "blue")

    def test_controller_noop_bulk_update_selected_markers_does_not_dirty_or_refresh(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        first_marker_id = self._selected_track_markers(controller)[0]["id"]
        controller.toggle_marker_selection(first_marker_id, False)
        self.assertEqual(controller.bulk_update_selected_markers("Scene", "scene", "violet"), 1)
        controller._set_dirty(False)
        marker_changes = []
        model_resets = []
        controller.selectedTrackMarkersChanged.connect(lambda: marker_changes.append(self._selected_track_markers(controller)))
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
        controller.selectedTrackMarkersChanged.connect(lambda: marker_changes.append(self._selected_track_markers(controller)))

        self.assertTrue(controller.delete_marker_from_selected_track(marker_id))

        self.assertEqual(len(marker_changes), 1)
        self.assertEqual(controller.selectedMarkerIds, [])

    def test_controller_delete_selected_markers_removes_multi_selection_with_history(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        marker_ids = [marker["id"] for marker in self._selected_track_markers(controller)]
        controller.toggle_marker_selection(marker_ids[0], False)
        controller.toggle_marker_selection(marker_ids[1], True)

        self.assertEqual(controller.delete_selected_markers(), 2)

        self.assertEqual(controller.selectedMarkerIds, [])
        self.assertFalse(any(marker.id in marker_ids for marker in controller._project.markers))
        self.assertTrue(controller.canUndo)

        self.assertTrue(controller.undo())
        self.assertTrue(all(self._marker_by_id(controller, marker_id) for marker_id in marker_ids))

    def test_controller_clears_stale_selected_marker_when_delete_returns_false(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        marker_id = controller.add_marker_to_selected_track(1.5, "Blackout")
        controller.toggle_marker_selection(marker_id, False)
        controller._project.markers[:] = [marker for marker in controller._project.markers if marker.id != marker_id]

        self.assertFalse(controller.delete_marker_from_selected_track(marker_id))

        self.assertEqual(controller.selectedMarkerIds, [])
        self.assertEqual(controller.lastError, "")

    def test_marker_summary_normalizes_invalid_color_key_to_default(self):
        from autolight.app_controller import AppController

        controller = AppController()
        self.addCleanup(controller.cleanup)
        controller.load_demo_project()
        editable_id = self._track_id_for_type(controller, "editable")
        controller.select_track(editable_id)
        first_marker_id = self._selected_track_markers(controller)[0]["id"]
        second_marker_id = self._selected_track_markers(controller)[1]["id"]
        first = self._marker_by_id(controller, first_marker_id)
        second = self._marker_by_id(controller, second_marker_id)
        first.metadata["color"] = "not-a-color"
        second.metadata["color"] = 42

        summaries = self._selected_track_markers(controller)

        self.assertEqual(len(summaries), 2)
        self.assertEqual(summaries[0]["color"], MARKER_COLOR_PALETTE["cyan"])
        self.assertEqual(summaries[1]["color"], MARKER_COLOR_PALETTE["cyan"])
        self.assertEqual(summaries[0]["colorKey"], "cyan")
        self.assertEqual(summaries[1]["colorKey"], "cyan")

    def test_qml_exposes_editable_marker_inspector(self):
        ui_root = Path(__file__).resolve().parents[1] / "UI"
        qml = "\n".join(
            [
                (ui_root / "Main.qml").read_text(encoding="utf-8"),
                (ui_root / "components" / "MarkerInspector.qml").read_text(encoding="utf-8"),
            ]
        )

        self.assertIn("id: inspectorPanel", qml)
        self.assertIn("markerTimestampField", qml)
        self.assertIn("markerDurationField", qml)
        self.assertIn("markerLabelField", qml)
        self.assertIn("appController.selectedTrackMarkers", qml)
        self.assertIn("inspectorPanel.selectedMarkerId", qml)
        self.assertIn("appController.add_marker_to_selected_track_with_duration", qml)
        self.assertIn("appController.delete_marker_from_selected_track(markerId)", qml)
        self.assertIn("appController.delete_selected_markers()", qml)
        self.assertIn("appController.selectedTrackIsEditable", qml)
        self.assertIn("DoubleValidator { bottom: 0.0 }", qml)
        self.assertIn("String(text).trim()", qml)
        self.assertIn("normalized.length === 0", qml)
        self.assertIn("validNonNegativeField(markerTimestampField.text)", qml)
        self.assertIn("validNonNegativeField(markerDurationField.text)", qml)
        self.assertIn(
            "enabled: inspectorPanel.appController.selectedTrackId.length > 0",
            qml,
        )
        self.assertIn(
            "enabled: inspectorPanel.selectedMarkerCount() > 0 && inspectorPanel.appController.selectedTrackIsEditable",
            qml,
        )

    def test_qml_exposes_marker_label_color_and_bulk_edit_controls(self):
        ui_root = Path(__file__).resolve().parents[1] / "UI"
        qml = "\n".join(
            [
                (ui_root / "Main.qml").read_text(encoding="utf-8"),
                (ui_root / "components" / "MarkerInspector.qml").read_text(encoding="utf-8"),
            ]
        )

        self.assertIn("id: markerColorPicker", qml)
        self.assertIn("id: markerCategoryField", qml)
        self.assertIn("appController.toggle_marker_selection", qml)
        self.assertIn("appController.update_selected_marker_with_duration", qml)
        self.assertIn("appController.bulk_update_selected_markers", qml)
        self.assertIn("modelData.color", qml)
        self.assertIn("modelData.selected", qml)
        self.assertIn("selectedMarkerIds.length", qml)
        self.assertIn("function syncMarkerEditorFromSelection()", qml)
        self.assertIn("onSelectedTrackMarkersChanged", qml)
        self.assertIn("markerInspector.syncMarkerEditorFromSelection()", qml)
        self.assertIn("No track selected", qml)
        self.assertNotIn("root.syncMarkerEditor(modelData)", qml)

    @staticmethod
    def _generated_track(project):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            source = import_audio_asset(project, audio_path)

        return add_generated_track(project, source.id, "Generated", "markers.fixed_interval", {}, "1", "markers.v1", "hash")

    @staticmethod
    def _source_track(project):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            return import_audio_asset(project, audio_path)

    @staticmethod
    def _editable_track(project):
        generated = EditableMarkerInspectorTest._generated_track(project)
        project.markers.append(Marker(id="marker_source", track_id=generated.id, timestamp=0.5))
        return create_editable_track_from_markers(project, generated.id, "Editable", ["marker_source"])

    @staticmethod
    def _track_id_for_type(controller, track_type: str) -> str:
        for track in controller._project.tracks:
            if track.type.value == track_type:
                return track.id
        raise AssertionError(f"track type not found: {track_type}")

    @staticmethod
    def _selected_track_markers(controller) -> list[dict]:
        return list(controller.selectedTrackMarkers)

    def _selected_marker_summary_by_id(self, controller, marker_id: str) -> dict:
        for item in self._selected_track_markers(controller):
            if item["id"] == marker_id:
                return item
        self.fail(f"marker summary not found: {marker_id}")

    def _marker_by_id(self, controller, marker_id: str) -> Marker:
        for marker in controller._project.markers:
            if marker.id == marker_id:
                return marker
        self.fail(f"marker not found: {marker_id}")

    def _project_track_by_id(self, project, track_id: str):
        for track in project.tracks:
            if track.id == track_id:
                return track
        self.fail(f"track not found: {track_id}")

    def _project_marker_by_id(self, project, marker_id: str) -> Marker:
        for marker in project.markers:
            if marker.id == marker_id:
                return marker
        self.fail(f"marker not found: {marker_id}")


if __name__ == "__main__":
    unittest.main()
