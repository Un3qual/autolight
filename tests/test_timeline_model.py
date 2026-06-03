import math
import tempfile
import unittest
from pathlib import Path

from PySide6.QtCore import QCoreApplication, QModelIndex, Qt

from autolight.project.models import CacheEntry, JobRun, Marker, ProjectDocument, ResultState, Track, TrackType
from autolight.project.store import add_generated_track, import_audio_asset, marker_display_color, new_project
from autolight.timeline.model import TimelineTrackModel
from tests.helpers import write_wav


class TimelineTrackModelTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication([])

    def test_model_exposes_track_roles_for_qml(self):
        with tempfile.TemporaryDirectory() as tmp:
            project, _source, generated = self._project_with_generated_track(Path(tmp))
            generated.result_state = ResultState.COMPLETE
            generated.error = "analysis failed"
            project.markers.append(Marker(id="marker_1", track_id=generated.id, timestamp=0.5))

            model = TimelineTrackModel()
            model.set_project(project)
            index = model.index(1, 0)

            self.assertEqual(
                model.roleNames(),
                {
                    model.role_for_name("trackId"): b"trackId",
                    model.role_for_name("name"): b"name",
                    model.role_for_name("trackType"): b"trackType",
                    model.role_for_name("resultState"): b"resultState",
                    model.role_for_name("markerCount"): b"markerCount",
                    model.role_for_name("markerSpans"): b"markerSpans",
                    model.role_for_name("error"): b"error",
                    model.role_for_name("activeJobId"): b"activeJobId",
                    model.role_for_name("jobState"): b"jobState",
                    model.role_for_name("jobProgress"): b"jobProgress",
                    model.role_for_name("waveformSamples"): b"waveformSamples",
                    model.role_for_name("waveformDurationSeconds"): b"waveformDurationSeconds",
                    model.role_for_name("cacheRefCount"): b"cacheRefCount",
                    model.role_for_name("artifactKinds"): b"artifactKinds",
                    model.role_for_name("editable"): b"editable",
                    model.role_for_name("visibleWaveformSamples"): b"visibleWaveformSamples",
                    model.role_for_name("waveformLevelBucketCount"): b"waveformLevelBucketCount",
                    model.role_for_name("parentTrackId"): b"parentTrackId",
                    model.role_for_name("depth"): b"depth",
                    model.role_for_name("hasChildren"): b"hasChildren",
                    model.role_for_name("expanded"): b"expanded",
                    model.role_for_name("childCount"): b"childCount",
                    model.role_for_name("visibleChildStateSummary"): b"visibleChildStateSummary",
                    model.role_for_name("treeError"): b"treeError",
                    model.role_for_name("visibleEnergySamples"): b"visibleEnergySamples",
                    model.role_for_name("visibleHarmonicColorSamples"): b"visibleHarmonicColorSamples",
                },
            )
            self.assertEqual(model.rowCount(), 2)
            self.assertEqual(model.data(index, model.role_for_name("trackId")), generated.id)
            self.assertEqual(model.data(index, model.role_for_name("name")), "Beats")
            self.assertEqual(model.data(index, model.role_for_name("trackType")), "generated")
            self.assertEqual(model.data(index, model.role_for_name("markerCount")), 1)
            self.assertEqual(
                model.data(index, model.role_for_name("markerSpans")),
                [
                    {
                        "id": "marker_1",
                        "timestamp": 0.5,
                        "duration": 0.0,
                        "label": "",
                        "category": "",
                        "color": "#67e8f9",
                        "selected": False,
                    }
                ],
            )
            self.assertEqual(model.data(index, model.role_for_name("resultState")), "complete")
            self.assertEqual(model.data(index, model.role_for_name("error")), "analysis failed")
            self.assertEqual(model.data(index, model.role_for_name("waveformSamples")), [])
            self.assertEqual(model.data(index, model.role_for_name("waveformDurationSeconds")), 0.0)
            self.assertEqual(model.data(index, model.role_for_name("cacheRefCount")), 0)
            self.assertEqual(model.data(index, model.role_for_name("artifactKinds")), "")
            self.assertEqual(model.data(index, Qt.ItemDataRole.DisplayRole), "Beats")

    def test_model_projects_tracks_as_expanded_tree_rows(self):
        project = new_project("Demo")
        source = Track(id="track_source", type=TrackType.SOURCE, name="Song", result_state=ResultState.COMPLETE)
        drums = Track(
            id="track_drums",
            type=TrackType.GENERATED,
            name="Drums",
            input_track_ids=[source.id],
            result_state=ResultState.COMPLETE,
        )
        onsets = Track(
            id="track_onsets",
            type=TrackType.GENERATED,
            name="Drum Onsets",
            input_track_ids=[drums.id],
            result_state=ResultState.STALE,
        )
        beat_grid = Track(
            id="track_beats",
            type=TrackType.GENERATED,
            name="Beat Grid",
            input_track_ids=[source.id],
            result_state=ResultState.COMPLETE,
        )
        project.tracks.extend([source, drums, onsets, beat_grid])

        model = TimelineTrackModel()
        model.set_project(project)

        self.assertEqual(model.rowCount(), 4)
        ids = [
            model.data(model.index(row, 0), model.role_for_name("trackId"))
            for row in range(model.rowCount())
        ]
        depths = [
            model.data(model.index(row, 0), model.role_for_name("depth"))
            for row in range(model.rowCount())
        ]

        self.assertEqual(ids, ["track_source", "track_drums", "track_onsets", "track_beats"])
        self.assertEqual(depths, [0, 1, 2, 1])
        self.assertEqual(model.data(model.index(0, 0), model.role_for_name("childCount")), 2)
        self.assertTrue(model.data(model.index(0, 0), model.role_for_name("hasChildren")))
        self.assertEqual(model.data(model.index(1, 0), model.role_for_name("parentTrackId")), "track_source")
        self.assertEqual(model.data(model.index(2, 0), model.role_for_name("parentTrackId")), "track_drums")
        self.assertEqual(model.data(model.index(1, 0), model.role_for_name("visibleChildStateSummary")), "stale: 1")

    def test_model_collapses_tree_rows_without_destroying_project_order(self):
        project = new_project("Demo")
        source = Track(id="track_source", type=TrackType.SOURCE, name="Song", result_state=ResultState.COMPLETE)
        child = Track(
            id="track_child",
            type=TrackType.GENERATED,
            name="Child",
            input_track_ids=[source.id],
            result_state=ResultState.COMPLETE,
        )
        sibling = Track(id="track_sibling", type=TrackType.SOURCE, name="Other", result_state=ResultState.COMPLETE)
        project.tracks.extend([source, child, sibling])

        model = TimelineTrackModel()
        model.set_project(project)
        self.assertTrue(model.set_track_expanded(source.id, False))

        ids = [
            model.data(model.index(row, 0), model.role_for_name("trackId"))
            for row in range(model.rowCount())
        ]

        self.assertEqual(ids, ["track_source", "track_sibling"])
        self.assertFalse(model.data(model.index(0, 0), model.role_for_name("expanded")))
        self.assertEqual([track.id for track in project.tracks], ["track_source", "track_child", "track_sibling"])

    def test_refresh_track_preserves_marker_cache_for_collapsed_child(self):
        project = new_project("Demo")
        source = Track(id="track_source", type=TrackType.SOURCE, name="Song", result_state=ResultState.COMPLETE)
        child = Track(
            id="track_child",
            type=TrackType.GENERATED,
            name="Child",
            input_track_ids=[source.id],
            result_state=ResultState.COMPLETE,
        )
        project.tracks.extend([source, child])
        project.markers.append(Marker(id="marker_child", track_id=child.id, timestamp=1.0))

        model = TimelineTrackModel()
        model.set_project(project)
        self.assertTrue(model.set_track_expanded(source.id, False))

        model.refresh_track(child.id)
        self.assertTrue(model.set_track_expanded(source.id, True))

        child_index = model.index(1, 0)
        self.assertEqual(model.data(child_index, model.role_for_name("trackId")), child.id)
        self.assertEqual(model.data(child_index, model.role_for_name("markerCount")), 1)

    def test_refresh_track_emits_summary_change_for_visible_ancestor_of_collapsed_child(self):
        project = new_project("Demo")
        source = Track(id="track_source", type=TrackType.SOURCE, name="Song", result_state=ResultState.COMPLETE)
        child = Track(
            id="track_child",
            type=TrackType.GENERATED,
            name="Child",
            input_track_ids=[source.id],
            result_state=ResultState.COMPLETE,
        )
        project.tracks.extend([source, child])

        model = TimelineTrackModel()
        model.set_project(project)
        self.assertTrue(model.set_track_expanded(source.id, False))
        emissions = []
        model.dataChanged.connect(
            lambda top_left, bottom_right, roles: emissions.append(
                (top_left.row(), bottom_right.row(), roles)
            )
        )

        child.result_state = ResultState.STALE
        model.refresh_track(child.id)

        summary_role = model.role_for_name("visibleChildStateSummary")
        ancestor_emissions = [emission for emission in emissions if emission[0:2] == (0, 0)]
        self.assertTrue(ancestor_emissions)
        ancestor_emission = ancestor_emissions[0]
        self.assertIn(summary_role, ancestor_emission[2])
        self.assertEqual(
            model.data(model.index(0, 0), summary_role),
            "stale: 1",
        )

    def test_set_project_preserves_collapsed_expansion_state(self):
        project = new_project("Demo")
        source = Track(id="track_source", type=TrackType.SOURCE, name="Song", result_state=ResultState.COMPLETE)
        child = Track(
            id="track_child",
            type=TrackType.GENERATED,
            name="Child",
            input_track_ids=[source.id],
            result_state=ResultState.COMPLETE,
        )
        project.tracks.extend([source, child])

        model = TimelineTrackModel()
        model.set_project(project)
        self.assertTrue(model.set_track_expanded(source.id, False))

        model.set_project(project)

        self.assertEqual(model.rowCount(), 1)
        self.assertEqual(model.data(model.index(0, 0), model.role_for_name("trackId")), source.id)
        self.assertFalse(model.data(model.index(0, 0), model.role_for_name("expanded")))

    def test_model_default_expansion_uses_only_first_input_parent(self):
        project = new_project("Demo")
        source = Track(id="track_source", type=TrackType.SOURCE, name="Song", result_state=ResultState.COMPLETE)
        sidechain = Track(
            id="track_sidechain",
            type=TrackType.GENERATED,
            name="Sidechain",
            result_state=ResultState.COMPLETE,
        )
        child = Track(
            id="track_child",
            type=TrackType.GENERATED,
            name="Child",
            input_track_ids=[source.id, sidechain.id],
            result_state=ResultState.COMPLETE,
        )
        project.tracks.extend([source, sidechain, child])

        model = TimelineTrackModel()
        model.set_project(project)

        self.assertEqual(model.expanded_track_ids(), [source.id])
        self.assertTrue(model.data(model.index(0, 0), model.role_for_name("expanded")))
        self.assertFalse(model.data(model.index(2, 0), model.role_for_name("expanded")))

    def test_model_renders_missing_parent_as_problem_root_row(self):
        project = new_project("Demo")
        orphan = Track(
            id="track_orphan",
            type=TrackType.GENERATED,
            name="Orphan",
            input_track_ids=["missing_parent"],
            result_state=ResultState.COMPLETE,
        )
        project.tracks.append(orphan)

        model = TimelineTrackModel()
        model.set_project(project)

        self.assertEqual(model.rowCount(), 1)
        self.assertEqual(model.data(model.index(0, 0), model.role_for_name("depth")), 0)
        self.assertEqual(
            model.data(model.index(0, 0), model.role_for_name("treeError")),
            "missing parent: missing_parent",
        )

    def test_model_surfaces_cycle_as_problem_root_row(self):
        project = new_project("Demo")
        track_a = Track(
            id="track_a",
            type=TrackType.GENERATED,
            name="A",
            input_track_ids=["track_b"],
            result_state=ResultState.COMPLETE,
        )
        track_b = Track(
            id="track_b",
            type=TrackType.GENERATED,
            name="B",
            input_track_ids=["track_a"],
            result_state=ResultState.COMPLETE,
        )
        project.tracks.extend([track_a, track_b])

        model = TimelineTrackModel()
        model.set_project(project)

        self.assertGreater(model.rowCount(), 0)
        errors = [
            model.data(model.index(row, 0), model.role_for_name("treeError"))
            for row in range(model.rowCount())
        ]
        self.assertIn("cycle detected", errors)

    def test_model_exposes_editability_and_marker_duration(self):
        project = new_project("Demo")
        editable = Track(
            id="track_edit",
            type=TrackType.EDITABLE,
            name="Editable",
            result_state=ResultState.COMPLETE,
        )
        project.tracks.append(editable)
        project.markers.append(
            Marker(id="marker_1", track_id=editable.id, timestamp=1.0, duration=0.5)
        )
        model = TimelineTrackModel()
        model.set_project(project)

        index = model.index(0, 0)
        self.assertTrue(model.data(index, model.role_for_name("editable")))
        spans = model.data(index, model.role_for_name("markerSpans"))
        self.assertEqual(spans[0]["duration"], 0.5)
        self.assertFalse(spans[0]["selected"])

    def test_model_exposes_visible_waveform_samples(self):
        project = new_project("Demo")
        waveform = Track(
            id="track_wave",
            type=TrackType.GENERATED,
            name="Waveform",
            transform_id="waveform.summary",
            result_state=ResultState.COMPLETE,
            provenance={
                "visible_waveform": {
                    "duration": 2.0,
                    "level_bucket_count": 8,
                    "samples": [{"time": 0.0, "peak": 0.2, "rms": 0.1}],
                }
            },
        )
        project.tracks.append(waveform)
        project.cache_entries.append(
            CacheEntry(
                id="cache_1",
                dependency_hash="dep",
                artifact_kind="waveform",
                path="waveform.json",
                created_at="",
                transform_version="1",
            )
        )
        waveform.cache_refs = ["cache_1"]
        model = TimelineTrackModel()
        model.set_project(project)

        index = model.index(0, 0)
        visible = model.data(index, model.role_for_name("visibleWaveformSamples"))
        self.assertEqual(visible[0]["time"], 0.0)
        self.assertEqual(model.data(index, model.role_for_name("waveformLevelBucketCount")), 8)

    def test_visible_waveform_roles_hide_incomplete_or_invalid_cache_results(self):
        cases = [
            (ResultState.STALE, "valid", ["cache_1"]),
            (ResultState.PENDING, "valid", ["cache_1"]),
            (ResultState.COMPLETE, "invalid", ["cache_1"]),
            (ResultState.COMPLETE, "valid", []),
        ]

        for result_state, validation_status, cache_refs in cases:
            with self.subTest(
                result_state=result_state,
                validation_status=validation_status,
                cache_refs=cache_refs,
            ):
                project = new_project("Demo")
                waveform = Track(
                    id="track_wave",
                    type=TrackType.GENERATED,
                    name="Waveform",
                    transform_id="waveform.summary",
                    result_state=result_state,
                    cache_refs=list(cache_refs),
                    provenance={
                        "visible_waveform": {
                            "duration": 2.0,
                            "level_bucket_count": 8,
                            "samples": [{"time": 0.0, "peak": 0.2, "rms": 0.1}],
                        }
                    },
                )
                project.tracks.append(waveform)
                project.cache_entries.append(
                    CacheEntry(
                        id="cache_1",
                        dependency_hash="dep",
                        artifact_kind="waveform",
                        path="waveform.json",
                        created_at="",
                        transform_version="1",
                        validation_status=validation_status,
                    )
                )
                model = TimelineTrackModel()
                model.set_project(project)

                index = model.index(0, 0)
                self.assertEqual(
                    model.data(index, model.role_for_name("visibleWaveformSamples")),
                    [],
                )
                self.assertEqual(
                    model.data(index, model.role_for_name("waveformLevelBucketCount")),
                    0,
                )

    def test_visible_waveform_samples_returns_copies(self):
        project = new_project("Demo")
        waveform = Track(
            id="track_wave",
            type=TrackType.GENERATED,
            name="Waveform",
            transform_id="waveform.summary",
            result_state=ResultState.COMPLETE,
            cache_refs=["cache_1"],
            provenance={
                "visible_waveform": {
                    "duration": 2.0,
                    "level_bucket_count": 8,
                    "samples": [{"time": 0.0, "peak": 0.2, "rms": 0.1}],
                }
            },
        )
        project.tracks.append(waveform)
        project.cache_entries.append(
            CacheEntry(
                id="cache_1",
                dependency_hash="dep",
                artifact_kind="waveform",
                path="waveform.json",
                created_at="",
                transform_version="1",
            )
        )
        model = TimelineTrackModel()
        model.set_project(project)

        visible = model.data(model.index(0, 0), model.role_for_name("visibleWaveformSamples"))
        visible[0]["peak"] = 0.99
        visible.append({"time": 1.0, "peak": 1.0, "rms": 1.0})

        provenance_samples = waveform.provenance["visible_waveform"]["samples"]
        self.assertEqual(
            provenance_samples,
            [{"time": 0.0, "peak": 0.2, "rms": 0.1}],
        )

    def test_model_exposes_visible_analysis_samples_for_complete_valid_artifacts(self):
        project = new_project("Demo")
        energy = Track(
            id="track_energy",
            type=TrackType.GENERATED,
            name="Energy",
            result_state=ResultState.COMPLETE,
            cache_refs=["cache_energy"],
            provenance={
                "visible_energy": {
                    "artifact_kind": "energy",
                    "cache_ref": "cache_energy",
                    "kind": "energy",
                    "frames": [{"time": 0.0, "intensity": 0.5}],
                }
            },
        )
        harmonic = Track(
            id="track_harmony",
            type=TrackType.GENERATED,
            name="Harmony",
            result_state=ResultState.COMPLETE,
            cache_refs=["cache_harmony"],
            provenance={
                "visible_harmonic_color": {
                    "artifact_kind": "harmonic-color",
                    "cache_ref": "cache_harmony",
                    "kind": "harmonic-color",
                    "frames": [{"time": 0.0, "color": "hsl(0, 72%, 58%)"}],
                }
            },
        )
        project.tracks.extend([energy, harmonic])
        project.cache_entries.extend(
            [
                CacheEntry("cache_energy", "dep", "energy", "energy.json", "", "1"),
                CacheEntry("cache_harmony", "dep", "harmonic-color", "harmony.json", "", "1"),
            ]
        )
        model = TimelineTrackModel()
        model.set_project(project)

        self.assertEqual(
            model.data(model.index(0, 0), model.role_for_name("visibleEnergySamples"))[0]["intensity"],
            0.5,
        )
        self.assertEqual(
            model.data(model.index(1, 0), model.role_for_name("visibleHarmonicColorSamples"))[0]["color"],
            "hsl(0, 72%, 58%)",
        )

    def test_visible_analysis_roles_hide_mismatched_artifact_metadata(self):
        project = new_project("Demo")
        energy = Track(
            id="track_energy",
            type=TrackType.GENERATED,
            name="Energy",
            result_state=ResultState.COMPLETE,
            cache_refs=["cache_energy_current"],
            provenance={
                "visible_energy": {
                    "artifact_kind": "energy",
                    "cache_ref": "cache_energy_old",
                    "kind": "energy",
                    "frames": [{"time": 0.0, "intensity": 0.5}],
                }
            },
        )
        project.tracks.append(energy)
        project.cache_entries.append(
            CacheEntry("cache_energy_current", "dep", "energy", "energy.json", "", "1")
        )
        model = TimelineTrackModel()
        model.set_project(project)

        self.assertEqual(
            model.data(model.index(0, 0), model.role_for_name("visibleEnergySamples")),
            [],
        )

    def test_waveform_level_bucket_count_returns_zero_for_malformed_values(self):
        malformed_values = [float("inf"), math.nan, {"bad": "type"}]
        malformed_visibles = [{"level_bucket_count": value} for value in malformed_values]
        malformed_visibles.append({})

        for visible in malformed_visibles:
            with self.subTest(visible=visible):
                project = new_project("Demo")
                project.tracks.append(
                    Track(
                        id="track_wave",
                        type=TrackType.GENERATED,
                        name="Waveform",
                        transform_id="waveform.summary",
                        result_state=ResultState.COMPLETE,
                        cache_refs=["cache_1"],
                        provenance={"visible_waveform": visible},
                    )
                )
                project.cache_entries.append(
                    CacheEntry(
                        id="cache_1",
                        dependency_hash="dep",
                        artifact_kind="waveform",
                        path="waveform.json",
                        created_at="",
                        transform_version="1",
                    )
                )
                model = TimelineTrackModel()
                model.set_project(project)

                self.assertEqual(
                    model.data(
                        model.index(0, 0),
                        model.role_for_name("waveformLevelBucketCount"),
                    ),
                    0,
                )

    def test_model_exposes_latest_job_state_progress_and_id(self):
        project = ProjectDocument(id="project_1", name="Demo")
        track = Track(id="track_1", type=TrackType.GENERATED, name="Beats")
        project.tracks.append(track)
        project.job_runs.append(
            JobRun(
                id="job_1",
                track_id="track_1",
                transform_id="markers.fixed_interval",
                parameters_hash="hash",
                state=ResultState.RUNNING,
                progress=0.25,
            )
        )
        model = TimelineTrackModel()
        model.set_project(project)

        index = model.index(0, 0)

        self.assertEqual(model.data(index, model.role_for_name("activeJobId")), "job_1")
        self.assertEqual(model.data(index, model.role_for_name("jobState")), "running")
        self.assertEqual(model.data(index, model.role_for_name("jobProgress")), 0.25)

    def test_waveform_roles_are_hidden_when_cache_is_invalid(self):
        project = ProjectDocument(id="project_1", name="Demo")
        project.tracks.append(
            Track(
                id="track_waveform",
                type=TrackType.GENERATED,
                name="Waveform",
                transform_id="waveform.summary",
                result_state=ResultState.COMPLETE,
                cache_refs=["cache_waveform"],
                provenance={
                    "waveform_samples": [{"peak": 1.0, "rms": 1.0}],
                    "waveform_duration_seconds": 1.25,
                },
            )
        )
        project.cache_entries.append(
            CacheEntry(
                id="cache_waveform",
                dependency_hash="dep",
                artifact_kind="waveform",
                path="waveform/cache_waveform.bin",
                created_at="",
                transform_version="1",
                validation_status="invalid",
            )
        )
        model = TimelineTrackModel()
        model.set_project(project)

        index = model.index(0, 0)
        samples = model.data(index, model.role_for_name("waveformSamples"))
        duration = model.data(index, model.role_for_name("waveformDurationSeconds"))

        self.assertEqual(samples, [])
        self.assertEqual(duration, 0.0)

    def test_waveform_duration_seconds_is_exposed_for_complete_valid_waveform_track(self):
        project = ProjectDocument(id="project_1", name="Demo")
        project.tracks.append(
            Track(
                id="track_waveform",
                type=TrackType.GENERATED,
                name="Waveform",
                transform_id="waveform.summary",
                result_state=ResultState.COMPLETE,
                cache_refs=["cache_waveform"],
                provenance={"waveform_duration_seconds": 1.25},
            )
        )
        project.cache_entries.append(
            CacheEntry(
                id="cache_waveform",
                dependency_hash="dep",
                artifact_kind="waveform",
                path="waveform/cache_waveform.bin",
                created_at="",
                transform_version="1",
                validation_status="valid",
            )
        )
        model = TimelineTrackModel()
        model.set_project(project)

        duration = model.data(model.index(0, 0), model.role_for_name("waveformDurationSeconds"))

        self.assertEqual(duration, 1.25)

    def test_marker_spans_are_sorted_by_timestamp_for_timeline_projection(self):
        with tempfile.TemporaryDirectory() as tmp:
            project, _source, generated = self._project_with_generated_track(Path(tmp))
            project.markers.extend(
                [
                    Marker(id="marker_late", track_id=generated.id, timestamp=3.0, duration=0.25, label="Late"),
                    Marker(id="marker_early", track_id=generated.id, timestamp=0.75, label="Early"),
                ]
            )
            model = TimelineTrackModel()
            model.set_project(project)

            spans = model.data(model.index(1, 0), model.role_for_name("markerSpans"))

            self.assertEqual([span["id"] for span in spans], ["marker_early", "marker_late"])
            self.assertEqual([span["timestamp"] for span in spans], [0.75, 3.0])
            self.assertEqual([span["duration"] for span in spans], [0.0, 0.25])

    def test_marker_spans_resolve_marker_color_metadata(self):
        with tempfile.TemporaryDirectory() as tmp:
            project, _source, generated = self._project_with_generated_track(Path(tmp))
            marker = Marker(
                id="marker_amber",
                track_id=generated.id,
                timestamp=0.5,
                label="Look",
                metadata={"color": "amber"},
            )
            project.markers.append(marker)
            model = TimelineTrackModel()
            model.set_project(project)

            span = model.data(model.index(1, 0), model.role_for_name("markerSpans"))[0]

            self.assertEqual(span["color"], marker_display_color(marker))

    def test_marker_roles_use_cached_track_index(self):
        with tempfile.TemporaryDirectory() as tmp:
            project, _source, generated = self._project_with_generated_track(Path(tmp))
            project.markers.append(Marker(id="marker_1", track_id=generated.id, timestamp=0.5))
            model = TimelineTrackModel()
            model.set_project(project)
            project.markers = RaisingMarkerList(project.markers)
            index = model.index(1, 0)

            self.assertEqual(model.data(index, model.role_for_name("markerCount")), 1)
            self.assertEqual(
                [span["id"] for span in model.data(index, model.role_for_name("markerSpans"))],
                ["marker_1"],
            )

    def test_refresh_track_rebuilds_cached_marker_index_for_track(self):
        with tempfile.TemporaryDirectory() as tmp:
            project, _source, generated = self._project_with_generated_track(Path(tmp))
            model = TimelineTrackModel()
            model.set_project(project)
            index = model.index(1, 0)
            self.assertEqual(model.data(index, model.role_for_name("markerCount")), 0)

            project.markers.append(Marker(id="marker_1", track_id=generated.id, timestamp=0.5))
            model.refresh_track(generated.id)

            self.assertEqual(model.data(index, model.role_for_name("markerCount")), 1)

    def test_refresh_track_emits_data_changed_for_existing_track(self):
        with tempfile.TemporaryDirectory() as tmp:
            project, _source, generated = self._project_with_generated_track(Path(tmp))
            model = TimelineTrackModel()
            model.set_project(project)
            emissions = []
            model.dataChanged.connect(
                lambda top_left, bottom_right, roles: emissions.append(
                    (top_left.row(), bottom_right.row(), roles)
                )
            )

            model.refresh_track(generated.id)

            track_emissions = [emission for emission in emissions if emission[0:2] == (1, 1)]
            self.assertTrue(track_emissions)
            track_emission = track_emissions[0]
            self.assertIn(model.role_for_name("resultState"), track_emission[2])
            self.assertIn(model.role_for_name("markerSpans"), track_emission[2])

    def test_role_names_returns_copy(self):
        model = TimelineTrackModel()
        role_names = model.roleNames()

        role_names[model.role_for_name("name")] = b"changed"

        self.assertEqual(model.roleNames()[model.role_for_name("name")], b"name")

    def test_role_lookup_uses_cached_reverse_map(self):
        model = TimelineTrackModel()
        name_role = model.role_for_name("name")
        model.ROLE_NAMES = RaisingRoleNames(model.ROLE_NAMES)

        self.assertEqual(model.role_for_name("name"), name_role)

    def test_invalid_indexes_return_none(self):
        with tempfile.TemporaryDirectory() as tmp:
            project, _source, _generated = self._project_with_generated_track(Path(tmp))
            model = TimelineTrackModel()
            model.set_project(project)

            self.assertIsNone(model.data(QModelIndex(), model.role_for_name("name")))
            self.assertIsNone(model.data(model.createIndex(0, 1), model.role_for_name("name")))
            self.assertIsNone(model.data(model.createIndex(model.rowCount(), 0), model.role_for_name("name")))

            other_model = TimelineTrackModel()
            other_model.set_project(project)
            self.assertIsNone(model.data(other_model.index(0, 0), model.role_for_name("name")))

            stale_index = model.index(1, 0)
            model.set_project(new_project("Empty"))
            self.assertIsNone(model.data(stale_index, model.role_for_name("name")))

    def test_same_row_stale_index_after_reset_returns_none(self):
        with tempfile.TemporaryDirectory() as tmp:
            project_a, _source_a, _generated_a = self._project_with_generated_track(
                Path(tmp) / "project_a",
                generated_name="Beats A",
            )
            project_b, _source_b, _generated_b = self._project_with_generated_track(
                Path(tmp) / "project_b",
                generated_name="Beats B",
            )
            model = TimelineTrackModel()
            name_role = model.role_for_name("name")

            model.set_project(project_a)
            stale_index = model.index(1, 0)

            model.set_project(project_b)

            self.assertIsNone(model.data(stale_index, name_role))
            self.assertEqual(model.data(model.index(1, 0), name_role), "Beats B")

    def test_constructor_accepts_optional_parent(self):
        parent = QCoreApplication.instance()

        model = TimelineTrackModel(parent=parent)

        self.assertIs(model.parent(), parent)

    def _project_with_generated_track(self, tmp: Path, generated_name: str = "Beats"):
        tmp.mkdir(parents=True, exist_ok=True)
        audio_path = tmp / "song.wav"
        write_wav(audio_path)
        project = new_project("Demo")
        source = import_audio_asset(project, audio_path)
        generated = add_generated_track(
            project,
            source.id,
            generated_name,
            "markers.fixed_interval",
            {},
            "1",
            "markers.v1",
            "dep",
        )
        return project, source, generated


class RaisingMarkerList(list):
    def __iter__(self):
        raise AssertionError("marker list should not be scanned")


class RaisingRoleNames(dict):
    def items(self):
        raise AssertionError("role names should not be scanned")


if __name__ == "__main__":
    unittest.main()
