import json
import tempfile
import unittest
import wave
from pathlib import Path
from unittest.mock import patch

from PySide6.QtCore import QCoreApplication

import soundfile
from autolight.analysis import waveform as waveform_module
from autolight.analysis.builtin import MAX_WAVEFORM_BUCKETS, register_builtin_transforms
from autolight.analysis.registry import TransformCancelled, TransformContext, TransformRegistry
from autolight.analysis.waveform import build_waveform_summary
from autolight.app.waveform_lod import WaveformLodStore


def write_wav(path: Path) -> None:
    samples = [0, 1000, -1000, 2000, -2000, 0, 500, -500]
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(8)
        handle.writeframes(b"".join(sample.to_bytes(2, "little", signed=True) for sample in samples))


def write_empty_wav(path: Path) -> None:
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(8)


class WaveformSummaryTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication([])

    def test_build_waveform_summary_returns_normalized_buckets(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            output_path = Path(tmp) / "waveform.json"
            write_wav(audio_path)

            build_waveform_summary(audio_path, output_path, buckets=4)
            payload = json.loads(output_path.read_text(encoding="utf-8"))

        self.assertEqual(payload["version"], 2)
        self.assertEqual(payload["sample_rate"], 8)
        self.assertEqual(len(payload["samples"]), 4)
        self.assertTrue(all(0.0 <= item["peak"] <= 1.0 for item in payload["samples"]))
        self.assertTrue(all(0.0 <= item["rms"] <= 1.0 for item in payload["samples"]))

    def test_build_waveform_summary_writes_pyramid_levels(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            output_path = Path(tmp) / "waveform.json"
            write_wav(audio_path)

            build_waveform_summary(audio_path, output_path, buckets=2)
            payload = json.loads(output_path.read_text(encoding="utf-8"))

        self.assertEqual(payload["version"], 2)
        self.assertIn("levels", payload)
        self.assertGreaterEqual(len(payload["levels"]), 2)
        self.assertEqual(payload["levels"][0]["bucket_count"], 2)
        self.assertGreater(payload["levels"][-1]["bucket_count"], payload["levels"][0]["bucket_count"])

    def test_build_waveform_summary_streams_audio_once_for_lod_levels(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            output_path = Path(tmp) / "waveform.json"
            write_wav(audio_path)

            with patch(
                "autolight.analysis.waveform._summarize_samples",
                wraps=waveform_module._summarize_samples,
            ) as summarize_samples:
                build_waveform_summary(audio_path, output_path, buckets=2)

        self.assertEqual(summarize_samples.call_count, 1)

    def test_build_waveform_summary_zero_frame_audio_has_consistent_empty_levels(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "empty.wav"
            output_path = Path(tmp) / "waveform.json"
            write_empty_wav(audio_path)

            build_waveform_summary(audio_path, output_path, buckets=4)
            payload = json.loads(output_path.read_text(encoding="utf-8"))

        self.assertEqual(payload["duration"], 0.0)
        self.assertEqual(payload["samples"], [])
        self.assertEqual(payload["levels"], [])

    def test_waveform_lod_selects_more_detail_when_zoomed_in(self):
        payload = {
            "version": 2,
            "duration": 8.0,
            "levels": [
                {"bucket_count": 8, "samples": [{"peak": 0.1, "rms": 0.05}] * 8},
                {"bucket_count": 64, "samples": [{"peak": 0.2, "rms": 0.10}] * 64},
            ],
        }
        store = WaveformLodStore()

        overview = store.visible_samples(payload, scroll_seconds=0.0, visible_seconds=1.0, pixels_per_second=12.0)
        detail = store.visible_samples(payload, scroll_seconds=0.0, visible_seconds=1.0, pixels_per_second=200.0)

        self.assertEqual(overview["level_bucket_count"], 8)
        self.assertEqual(detail["level_bucket_count"], 64)
        self.assertLessEqual(len(detail["samples"]), 16)

    def test_waveform_lod_scales_partial_window_selection_to_file_duration(self):
        payload = {
            "version": 2,
            "duration": 8.0,
            "levels": [
                {"bucket_count": 8, "samples": [{"peak": 0.1, "rms": 0.05}] * 8},
                {"bucket_count": 64, "samples": [{"peak": 0.2, "rms": 0.10}] * 64},
            ],
        }
        store = WaveformLodStore()

        visible = store.visible_samples(payload, scroll_seconds=0.0, visible_seconds=1.0, pixels_per_second=48.0)

        self.assertEqual(visible["level_bucket_count"], 64)
        self.assertGreaterEqual(len(visible["samples"]), 8)

    def test_waveform_lod_normalizes_nonfinite_bucket_count(self):
        payload = {
            "version": 2,
            "duration": 10.0,
            "levels": [
                {"bucket_count": float("inf"), "samples": [{"peak": 0.1, "rms": 0.05}] * 10},
            ],
        }
        store = WaveformLodStore()

        visible = store.visible_samples(payload, scroll_seconds=0.0, visible_seconds=1.0, pixels_per_second=48.0)

        self.assertEqual(visible["level_bucket_count"], 10)
        self.assertGreater(len(visible["samples"]), 0)

    def test_waveform_lod_normalizes_mismatched_bucket_count_to_samples(self):
        samples = [{"peak": index / 10.0, "rms": 0.05} for index in range(10)]
        payload = {
            "version": 2,
            "duration": 10.0,
            "levels": [
                {"bucket_count": 100, "samples": samples},
            ],
        }
        store = WaveformLodStore()

        visible = store.visible_samples(payload, scroll_seconds=9.0, visible_seconds=1.0, pixels_per_second=48.0)

        self.assertEqual(visible["level_bucket_count"], 10)
        self.assertGreater(len(visible["samples"]), 0)
        self.assertEqual(visible["samples"][-1]["peak"], 0.9)
        self.assertEqual(visible["samples"][-1]["time"], 9.0)

    def test_waveform_lod_reads_legacy_single_sample_payload(self):
        payload = {
            "version": 1,
            "duration": 1.0,
            "samples": [{"peak": 0.25, "rms": 0.10}],
        }
        store = WaveformLodStore()

        visible = store.visible_samples(payload, scroll_seconds=0.0, visible_seconds=1.0, pixels_per_second=96.0)

        self.assertEqual(visible["level_bucket_count"], 1)
        self.assertEqual(visible["samples"][0]["peak"], 0.25)

    def test_build_waveform_summary_does_not_use_whole_file_read(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            output_path = Path(tmp) / "waveform.json"
            write_wav(audio_path)

            with patch("autolight.analysis.waveform.soundfile.read", side_effect=AssertionError("whole file read")):
                build_waveform_summary(audio_path, output_path, buckets=4)

            self.assertTrue(output_path.exists())

    def test_build_waveform_summary_falls_back_when_soundfile_rejects_container(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.unsupported"
            output_path = Path(tmp) / "waveform.json"
            audio_path.write_bytes(b"unsupported container")

            with (
                patch(
                    "autolight.analysis.waveform.soundfile.SoundFile",
                    side_effect=soundfile.SoundFileError("unsupported"),
                ),
                patch("audioread.audio_open", return_value=FakeAudioRead()),
            ):
                build_waveform_summary(audio_path, output_path, buckets=2)

            payload = json.loads(output_path.read_text(encoding="utf-8"))

        self.assertEqual(payload["sample_rate"], 4)
        self.assertEqual(len(payload["samples"]), 2)
        self.assertGreater(payload["samples"][0]["peak"], 0.0)

    def test_build_waveform_summary_checks_cancel_between_reads(self):
        cancel_checks = 0

        def cancel_after_first_check():
            nonlocal cancel_checks
            cancel_checks += 1
            return cancel_checks > 1

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            output_path = Path(tmp) / "waveform.json"
            write_wav(audio_path)

            with self.assertRaises(TransformCancelled):
                build_waveform_summary(
                    audio_path,
                    output_path,
                    buckets=4,
                    cancel_requested=cancel_after_first_check,
                )

            self.assertFalse(output_path.exists())
        self.assertGreater(cancel_checks, 1)

    def test_waveform_summary_transform_writes_artifact(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            artifact_dir = root / "artifacts"
            write_wav(audio_path)
            transform = registry.get("waveform.summary", version="1")
            result = transform.run(
                TransformContext(
                    artifact_dir=artifact_dir,
                    cancel_requested=lambda: False,
                    progress=lambda value: None,
                ),
                {"audio_path": str(audio_path), "buckets": 4},
            )

        self.assertEqual(set(result.artifacts), {"waveform"})
        self.assertTrue(Path(result.artifacts["waveform"]).name.endswith(".json"))
        self.assertEqual(result.metadata["bucket_count"], 4)

    def test_waveform_summary_transform_clamps_bucket_count(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            artifact_dir = root / "artifacts"
            write_wav(audio_path)
            transform = registry.get("waveform.summary", version="1")
            result = transform.run(
                TransformContext(
                    artifact_dir=artifact_dir,
                    cancel_requested=lambda: False,
                    progress=lambda value: None,
                ),
                {"audio_path": str(audio_path), "buckets": MAX_WAVEFORM_BUCKETS + 100},
            )

        self.assertEqual(result.metadata["bucket_count"], MAX_WAVEFORM_BUCKETS)

    def test_waveform_summary_transform_cancels_before_loading_audio(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        with tempfile.TemporaryDirectory() as tmp:
            context = TransformContext(
                artifact_dir=Path(tmp) / "artifacts",
                cancel_requested=lambda: True,
                progress=lambda value: None,
            )

            with self.assertRaises(TransformCancelled):
                registry.get("waveform.summary", version="1").run(context, {"audio_path": "missing.wav"})

    def test_controller_loads_waveform_samples_after_job_completion(self):
        from autolight.app_controller import AppController
        from autolight.project.store import add_generated_track, import_audio_asset

        controller = AppController()
        self.addCleanup(controller.cleanup)

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            source = import_audio_asset(controller._project, audio_path)
            track = add_generated_track(
                controller._project,
                parent_track_id=source.id,
                name="Waveform",
                transform_id="waveform.summary",
                transform_params={"audio_path": str(audio_path), "buckets": 4},
                transform_version="1",
                output_schema="artifact.waveform.v1",
                dependency_hash="waveform-test",
            )
            controller.trackModel.set_project(controller._project)

            job_id = controller.run_track(track.id)
            controller._job_queue.wait(job_id, timeout=5)
            QCoreApplication.processEvents()

        model = controller.trackModel
        waveform_role = model.role_for_name("waveformSamples")
        waveform_duration_role = model.role_for_name("waveformDurationSeconds")
        visible_role = model.role_for_name("visibleWaveformSamples")
        visible_bucket_role = model.role_for_name("waveformLevelBucketCount")
        row = self._track_row(controller, track.id)
        samples = model.data(model.index(row, 0), waveform_role)
        duration = model.data(model.index(row, 0), waveform_duration_role)
        visible_samples = model.data(model.index(row, 0), visible_role)
        visible_bucket_count = model.data(model.index(row, 0), visible_bucket_role)

        self.assertEqual(len(samples), 4)
        self.assertIn("peak", samples[0])
        self.assertAlmostEqual(duration, 1.0)
        self.assertEqual(track.provenance["waveform_payload"]["version"], 2)
        self.assertEqual(visible_bucket_count, 8)
        self.assertIn("time", visible_samples[0])

    def test_controller_refreshes_visible_waveform_for_timeline_viewport_changes(self):
        from autolight.app_controller import AppController
        from autolight.project.models import CacheEntry, ResultState
        from autolight.project.store import add_generated_track, import_audio_asset

        controller = AppController()
        self.addCleanup(controller.cleanup)

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            source = import_audio_asset(controller._project, audio_path)
            controller._project.audio_assets[0].duration = 8.0
            track = add_generated_track(
                controller._project,
                parent_track_id=source.id,
                name="Waveform",
                transform_id="waveform.summary",
                transform_params={"buckets": 8},
                transform_version="1",
                output_schema="artifact.waveform.v1",
                dependency_hash="waveform-test",
            )

        track.result_state = ResultState.COMPLETE
        track.cache_refs = ["cache_waveform"]
        track.provenance["waveform_payload"] = {
            "version": 2,
            "duration": 8.0,
            "levels": [
                {"bucket_count": 8, "samples": [{"peak": 0.1, "rms": 0.05}] * 8},
                {"bucket_count": 256, "samples": [{"peak": 0.2, "rms": 0.10}] * 256},
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

        controller.set_timeline_zoom(24.0)
        overview = track.provenance["visible_waveform"]
        controller.set_timeline_zoom(240.0)
        zoomed = track.provenance["visible_waveform"]
        controller.set_timeline_visible_seconds(1.0)
        detail = track.provenance["visible_waveform"]
        controller.set_timeline_scroll_seconds(6.0)
        scrolled = track.provenance["visible_waveform"]

        self.assertEqual(overview["level_bucket_count"], 8)
        self.assertEqual(zoomed["level_bucket_count"], 256)
        self.assertEqual(detail["level_bucket_count"], 256)
        self.assertGreater(scrolled["samples"][0]["time"], detail["samples"][0]["time"])

    def test_controller_refresh_clears_visible_waveform_for_incomplete_or_invalid_cache(self):
        from autolight.app_controller import AppController
        from autolight.project.models import CacheEntry, ResultState
        from autolight.project.store import add_generated_track, import_audio_asset

        controller = AppController()
        self.addCleanup(controller.cleanup)

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            source = import_audio_asset(controller._project, audio_path)
            stale_track = add_generated_track(
                controller._project,
                parent_track_id=source.id,
                name="Stale Waveform",
                transform_id="waveform.summary",
                transform_params={"buckets": 8},
                transform_version="1",
                output_schema="artifact.waveform.v1",
                dependency_hash="waveform-stale",
            )
            invalid_cache_track = add_generated_track(
                controller._project,
                parent_track_id=source.id,
                name="Invalid Cache Waveform",
                transform_id="waveform.summary",
                transform_params={"buckets": 8},
                transform_version="1",
                output_schema="artifact.waveform.v1",
                dependency_hash="waveform-invalid",
            )

        stale_track.result_state = ResultState.STALE
        stale_track.cache_refs = ["cache_stale"]
        invalid_cache_track.result_state = ResultState.COMPLETE
        invalid_cache_track.cache_refs = ["cache_invalid"]
        payload = {
            "version": 2,
            "duration": 8.0,
            "levels": [
                {"bucket_count": 8, "samples": [{"peak": 0.1, "rms": 0.05}] * 8}
            ],
        }
        for track in (stale_track, invalid_cache_track):
            track.provenance["waveform_payload"] = payload
            track.provenance["visible_waveform"] = {
                "duration": 8.0,
                "level_bucket_count": 8,
                "samples": [{"time": 0.0, "peak": 0.1, "rms": 0.05}],
            }
        controller._project.cache_entries.extend(
            [
                CacheEntry(
                    id="cache_stale",
                    dependency_hash="dep",
                    artifact_kind="waveform",
                    path="waveform/cache_stale.bin",
                    created_at="",
                    transform_version="1",
                ),
                CacheEntry(
                    id="cache_invalid",
                    dependency_hash="dep",
                    artifact_kind="waveform",
                    path="waveform/cache_invalid.bin",
                    created_at="",
                    transform_version="1",
                    validation_status="invalid",
                ),
            ]
        )
        controller.trackModel.set_project(controller._project)
        emissions = []
        controller.trackModel.dataChanged.connect(
            lambda top_left, _bottom_right, _roles: emissions.append(top_left.row())
        )

        controller._refresh_visible_waveforms()

        self.assertNotIn("visible_waveform", stale_track.provenance)
        self.assertNotIn("visible_waveform", invalid_cache_track.provenance)
        self.assertEqual(
            sorted(emissions),
            sorted(
                [
                    self._track_row(controller, stale_track.id),
                    self._track_row(controller, invalid_cache_track.id),
                ]
            ),
        )

    def test_controller_zoom_refreshes_visible_waveform_once_when_scroll_changes(self):
        from autolight.app_controller import AppController
        from autolight.project.models import CacheEntry, ResultState
        from autolight.project.store import add_generated_track, import_audio_asset

        controller = AppController()
        self.addCleanup(controller.cleanup)

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            source = import_audio_asset(controller._project, audio_path)
            controller._project.audio_assets[0].duration = 100.0
            track = add_generated_track(
                controller._project,
                parent_track_id=source.id,
                name="Waveform",
                transform_id="waveform.summary",
                transform_params={"buckets": 8},
                transform_version="1",
                output_schema="artifact.waveform.v1",
                dependency_hash="waveform-test",
            )

        track.result_state = ResultState.COMPLETE
        track.cache_refs = ["cache_waveform"]
        track.provenance["waveform_payload"] = {
            "version": 2,
            "duration": 100.0,
            "levels": [
                {"bucket_count": 100, "samples": [{"peak": 0.2, "rms": 0.10}] * 100}
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
        controller.set_timeline_visible_seconds(10.0)
        controller.set_timeline_scroll_seconds(60.0)
        row = self._track_row(controller, track.id)
        emissions = []
        controller.trackModel.dataChanged.connect(
            lambda top_left, _bottom_right, _roles: emissions.append(top_left.row())
        )

        controller.set_timeline_zoom(200.0)

        self.assertEqual([item for item in emissions if item == row], [row])

    def test_controller_restores_waveform_samples_after_open_project(self):
        from autolight.app_controller import AppController
        from autolight.project.store import add_generated_track, import_audio_asset

        controller = AppController()
        self.addCleanup(controller.cleanup)

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            audio_path = root / "song.wav"
            project_path = root / "show.autolight"
            write_wav(audio_path)
            source = import_audio_asset(controller._project, audio_path)
            track = add_generated_track(
                controller._project,
                parent_track_id=source.id,
                name="Waveform",
                transform_id="waveform.summary",
                transform_params={"buckets": 4},
                transform_version="1",
                output_schema="artifact.waveform.v1",
                dependency_hash="waveform-test",
            )
            controller.trackModel.set_project(controller._project)
            job_id = controller.run_track(track.id)
            controller._job_queue.wait(job_id, timeout=5)
            QCoreApplication.processEvents()
            track.provenance.pop("waveform_samples", None)
            track.provenance.pop("waveform_duration_seconds", None)
            track.provenance.pop("waveform_payload", None)
            self.assertTrue(controller.save_project(str(project_path)))

            self.assertTrue(controller.open_project(str(project_path)))
            QCoreApplication.processEvents()

        model = controller.trackModel
        samples = model.data(model.index(self._track_row(controller, track.id), 0), model.role_for_name("waveformSamples"))
        duration = model.data(
            model.index(self._track_row(controller, track.id), 0),
            model.role_for_name("waveformDurationSeconds"),
        )
        self.assertEqual(len(samples), 4)
        self.assertAlmostEqual(duration, 1.0)

    def test_controller_clears_waveform_samples_when_artifact_cannot_be_loaded(self):
        from autolight.app_controller import AppController
        from autolight.project.models import CacheEntry, ResultState
        from autolight.project.store import add_generated_track, import_audio_asset

        controller = AppController()
        self.addCleanup(controller.cleanup)

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            source = import_audio_asset(controller._project, audio_path)
            track = add_generated_track(
                controller._project,
                parent_track_id=source.id,
                name="Waveform",
                transform_id="waveform.summary",
                transform_params={"buckets": 4},
                transform_version="1",
                output_schema="artifact.waveform.v1",
                dependency_hash="waveform-test",
            )
        track.result_state = ResultState.COMPLETE
        track.cache_refs = ["missing_waveform"]
        track.provenance["waveform_samples"] = [{"peak": 1.0, "rms": 1.0}]
        track.provenance["waveform_duration_seconds"] = 12.5
        track.provenance["waveform_payload"] = {"version": 2, "samples": []}
        track.provenance["visible_waveform"] = {
            "duration": 12.5,
            "level_bucket_count": 1,
            "samples": [],
        }
        controller._project.cache_entries.append(
            CacheEntry(
                id="missing_waveform",
                dependency_hash="dep",
                artifact_kind="waveform",
                path="waveform/missing.bin",
                created_at="",
                transform_version="1",
                size_bytes=10,
            )
        )

        controller._load_waveform_samples(track.id)

        self.assertNotIn("waveform_samples", track.provenance)
        self.assertNotIn("waveform_duration_seconds", track.provenance)
        self.assertNotIn("waveform_payload", track.provenance)
        self.assertNotIn("visible_waveform", track.provenance)

    def test_qml_mentions_waveform_samples_role(self):
        ui_root = Path(__file__).resolve().parents[1] / "UI"
        qml = "\n".join(
            [
                (ui_root / "components" / "TimelineLane.qml").read_text(encoding="utf-8"),
                (ui_root / "components" / "WaveformStrip.qml").read_text(encoding="utf-8"),
            ]
        )
        self.assertNotIn("waveformSamples", qml)
        self.assertIn("waveformDurationSeconds", qml)
        self.assertIn("visibleWaveformSamples", qml)
        self.assertIn("sample.peak", qml)
        self.assertIn("clip: true", qml)
        self.assertIn("root.timelineLeftPadding", qml)
        self.assertIn("scrollSeconds: root.appController.timelineScrollSeconds", qml)
        self.assertIn("pixelsPerSecond: root.appController.timelinePixelsPerSecond", qml)
        self.assertNotIn(
            "root.timelineX(index / Math.max(1, waveformSamples.length - 1) * appController.timelineDurationSeconds)",
            qml,
        )
        self.assertNotIn("model: waveformSamples", qml)
        self.assertIn("visible: root.visibleWaveformSamples.length > 0", qml)

    def test_qml_waveform_uses_peak_and_rms_layers(self):
        qml = (
            Path(__file__).resolve().parents[1] / "UI" / "components" / "WaveformStrip.qml"
        ).read_text(encoding="utf-8")

        self.assertIn("Canvas", qml)
        self.assertIn("sample.peak", qml)
        self.assertIn("sample.rms", qml)
        self.assertIn("ctx.strokeStyle = peakColor", qml)
        self.assertIn("ctx.strokeStyle = rmsColor", qml)

    def _track_row(self, controller, track_id: str) -> int:
        for index, track in enumerate(controller._project.tracks):
            if track.id == track_id:
                return index
        self.fail(f"track not found: {track_id}")


class FakeAudioRead:
    samplerate = 4
    channels = 1
    duration = 1.0

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, traceback):
        return False

    def __iter__(self):
        for sample in (0, 8000, -8000, 16000):
            yield sample.to_bytes(2, "little", signed=True)


if __name__ == "__main__":
    unittest.main()
