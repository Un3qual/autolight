import json
import tempfile
import unittest
import wave
from pathlib import Path

from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformContext, TransformRegistry
from autolight.analysis.waveform import build_waveform_summary


def write_wav(path: Path) -> None:
    samples = [0, 1000, -1000, 2000, -2000, 0, 500, -500]
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(8)
        handle.writeframes(b"".join(sample.to_bytes(2, "little", signed=True) for sample in samples))


class WaveformSummaryTest(unittest.TestCase):
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

        model = controller.trackModel
        waveform_role = model.role_for_name("waveformSamples")
        row = next(index for index, item in enumerate(controller._project.tracks) if item.id == track.id)
        samples = model.data(model.index(row, 0), waveform_role)

        self.assertEqual(len(samples), 4)
        self.assertIn("peak", samples[0])

    def test_qml_mentions_waveform_samples_role(self):
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")
        self.assertIn("waveformSamples", qml)
        self.assertIn("modelData.peak", qml)


if __name__ == "__main__":
    unittest.main()
