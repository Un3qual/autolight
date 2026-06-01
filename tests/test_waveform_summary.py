import json
import tempfile
import unittest
import wave
from pathlib import Path
from unittest.mock import patch

from PySide6.QtCore import QCoreApplication

from autolight.analysis.builtin import MAX_WAVEFORM_BUCKETS, register_builtin_transforms
from autolight.analysis.registry import TransformCancelled, TransformContext, TransformRegistry
from autolight.analysis.waveform import build_waveform_summary


def write_wav(path: Path) -> None:
    samples = [0, 1000, -1000, 2000, -2000, 0, 500, -500]
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(8)
        handle.writeframes(b"".join(sample.to_bytes(2, "little", signed=True) for sample in samples))


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

        self.assertEqual(payload["version"], 1)
        self.assertEqual(payload["sample_rate"], 8)
        self.assertEqual(len(payload["samples"]), 4)
        self.assertTrue(all(0.0 <= item["peak"] <= 1.0 for item in payload["samples"]))
        self.assertTrue(all(0.0 <= item["rms"] <= 1.0 for item in payload["samples"]))

    def test_build_waveform_summary_does_not_use_whole_file_read(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            output_path = Path(tmp) / "waveform.json"
            write_wav(audio_path)

            with patch("autolight.analysis.waveform.soundfile.read", side_effect=AssertionError("whole file read")):
                build_waveform_summary(audio_path, output_path, buckets=4)

            self.assertTrue(output_path.exists())

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
        row = self._track_row(controller, track.id)
        samples = model.data(model.index(row, 0), waveform_role)

        self.assertEqual(len(samples), 4)
        self.assertIn("peak", samples[0])

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
            self.assertTrue(controller.save_project(str(project_path)))

            self.assertTrue(controller.open_project(str(project_path)))
            QCoreApplication.processEvents()

        model = controller.trackModel
        samples = model.data(model.index(self._track_row(controller, track.id), 0), model.role_for_name("waveformSamples"))
        self.assertEqual(len(samples), 4)

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

    def test_qml_mentions_waveform_samples_role(self):
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")
        self.assertIn("waveformSamples", qml)
        self.assertIn("modelData.peak", qml)
        self.assertIn("clip: true", qml)
        self.assertIn("root.timelineLeftPadding", qml)
        self.assertIn("waveformSamples.length > 1", qml)

    def _track_row(self, controller, track_id: str) -> int:
        for index, track in enumerate(controller._project.tracks):
            if track.id == track_id:
                return index
        self.fail(f"track not found: {track_id}")


if __name__ == "__main__":
    unittest.main()
