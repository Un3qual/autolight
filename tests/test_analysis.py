import json
import tempfile
import unittest
from pathlib import Path

from autolight.analysis import TransformCancelled
from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformContext, TransformRegistry, TransformSpec


class CancelOnCall:
    def __init__(self, call_number):
        self.call_number = call_number
        self.calls = 0

    def __call__(self):
        self.calls += 1
        return self.calls >= self.call_number


def write_impulse_wav(path: Path) -> None:
    import wave

    sample_rate = 8000
    samples = []
    for index in range(sample_rate * 2):
        value = 18000 if index % (sample_rate // 2) == 0 else 0
        samples.append(value.to_bytes(2, "little", signed=True))
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(sample_rate)
        handle.writeframes(b"".join(samples))


class AnalysisRegistryTest(unittest.TestCase):
    def test_builtin_registry_contains_marker_and_expensive_transforms(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        self.assertIn("markers.fixed_interval", registry.ids())
        self.assertIn("stems.vocals_stand_in", registry.ids())

    def test_builtin_registry_contains_music_analysis_transforms(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        self.assertIn("music.beat_grid", registry.ids())
        self.assertIn("music.energy_profile", registry.ids())
        self.assertIn("music.harmonic_color", registry.ids())

    def test_music_analysis_transforms_write_expected_artifacts(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        for transform_id, artifact_kind in [
            ("music.beat_grid", "beat-grid"),
            ("music.energy_profile", "energy"),
            ("music.harmonic_color", "harmonic-color"),
        ]:
            with self.subTest(transform_id=transform_id):
                with tempfile.TemporaryDirectory() as tmp:
                    audio_path = Path(tmp) / "song.wav"
                    write_impulse_wav(audio_path)
                    transform = registry.get(transform_id, version="1")
                    result = transform.run(
                        TransformContext(
                            artifact_dir=Path(tmp) / "artifacts",
                            cancel_requested=lambda: False,
                            progress=lambda value: None,
                        ),
                        {"audio_path": str(audio_path), "max_frames": 32, "max_markers": 64},
                    )
                    artifact = Path(result.artifacts[artifact_kind])
                    payload = json.loads(artifact.read_text(encoding="utf-8"))

                self.assertEqual(payload["version"], 1)
                self.assertEqual(payload["kind"], artifact_kind)
                self.assertIn("settings", payload)

    def test_music_analysis_transform_cancels_before_loading_audio(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        transform = registry.get("music.energy_profile", version="1")

        with tempfile.TemporaryDirectory() as tmp, self.assertRaises(TransformCancelled):
            transform.run(
                TransformContext(
                    artifact_dir=Path(tmp) / "artifacts",
                    cancel_requested=lambda: True,
                    progress=lambda value: None,
                ),
                {"audio_path": str(Path(tmp) / "missing.wav")},
            )

    def test_music_analysis_transform_cancels_inside_analyzer(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        transform = registry.get("music.energy_profile", version="1")

        with tempfile.TemporaryDirectory() as tmp, self.assertRaises(TransformCancelled):
            transform.run(
                TransformContext(
                    artifact_dir=Path(tmp) / "artifacts",
                    cancel_requested=CancelOnCall(2),
                    progress=lambda value: None,
                ),
                {"audio_path": str(Path(tmp) / "missing.wav")},
            )

    def test_registry_get_supports_version_lookup(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        transform = registry.get("markers.fixed_interval", version="1")

        self.assertEqual(transform.version, "1")
        with self.assertRaises(ValueError):
            registry.get("markers.fixed_interval", version="2")

    def test_registry_supports_side_by_side_transform_versions(self):
        registry = TransformRegistry()
        version_1 = TransformSpec(
            id="markers.example",
            version="1",
            name="Example v1",
            input_schema="audio.v1",
            output_schema="markers.v1",
            estimated_cost="light",
            run=lambda context, params: None,
        )
        version_2 = TransformSpec(
            id="markers.example",
            version="2",
            name="Example v2",
            input_schema="audio.v1",
            output_schema="markers.v1",
            estimated_cost="light",
            run=lambda context, params: None,
        )

        registry.register(version_1)
        registry.register(version_2)

        self.assertIs(registry.get("markers.example", version="1"), version_1)
        self.assertIs(registry.get("markers.example", version="2"), version_2)
        self.assertEqual(registry.ids(), ["markers.example"])
        with self.assertRaisesRegex(ValueError, "multiple versions"):
            registry.get("markers.example")

    def test_fixed_interval_transform_returns_markers(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        transform = registry.get("markers.fixed_interval")

        with tempfile.TemporaryDirectory() as tmp:
            result = transform.run(
                TransformContext(
                    artifact_dir=Path(tmp),
                    cancel_requested=lambda: False,
                    progress=lambda value: None,
                ),
                {"duration": 2.0, "interval": 0.5},
            )

        self.assertEqual(
            [marker["timestamp"] for marker in result.markers],
            [0.0, 0.5, 1.0, 1.5, 2.0],
        )
        self.assertEqual(result.artifacts, {})

    def test_fixed_interval_reports_bounded_intermediate_progress(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        transform = registry.get("markers.fixed_interval")
        progress_values = []

        with tempfile.TemporaryDirectory() as tmp:
            transform.run(
                TransformContext(
                    artifact_dir=Path(tmp),
                    cancel_requested=lambda: False,
                    progress=progress_values.append,
                ),
                {"duration": 4.0, "interval": 1.0},
            )

        self.assertEqual(progress_values[-1], 1.0)
        self.assertTrue(any(0.0 < value < 1.0 for value in progress_values))
        self.assertTrue(all(0.0 <= value <= 1.0 for value in progress_values))

    def test_fixed_interval_cancellation_raises_transform_cancelled(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        transform = registry.get("markers.fixed_interval")

        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(TransformCancelled):
                transform.run(
                    TransformContext(
                        artifact_dir=Path(tmp),
                        cancel_requested=lambda: True,
                        progress=lambda value: None,
                    ),
                    {"duration": 2.0, "interval": 0.5},
                )

    def test_fixed_interval_rejects_non_finite_values_before_looping(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        transform = registry.get("markers.fixed_interval")

        for params in [
            {"duration": float("nan"), "interval": 0.5},
            {"duration": 1.0, "interval": float("inf")},
            {"duration": float("inf"), "interval": 0.5},
        ]:
            with self.subTest(params=params):
                with tempfile.TemporaryDirectory() as tmp:
                    with self.assertRaisesRegex(ValueError, "finite"):
                        transform.run(
                            TransformContext(
                                artifact_dir=Path(tmp),
                                cancel_requested=CancelOnCall(1),
                                progress=lambda value: None,
                            ),
                            params,
                        )

    def test_fixed_interval_rejects_unbounded_marker_generation_before_looping(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        transform = registry.get("markers.fixed_interval")

        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(ValueError, "too many markers"):
                transform.run(
                    TransformContext(
                        artifact_dir=Path(tmp),
                        cancel_requested=CancelOnCall(1),
                        progress=lambda value: None,
                    ),
                    {"duration": 1_000_000.0, "interval": 0.001},
                )

    def test_vocal_stand_in_writes_artifact(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        transform = registry.get("stems.vocals_stand_in")
        progress_values = []

        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp) / "source.wav"
            source.write_bytes(b"test audio bytes")
            result = transform.run(
                TransformContext(
                    artifact_dir=Path(tmp),
                    cancel_requested=lambda: False,
                    progress=progress_values.append,
                ),
                {"audio_path": str(source), "label": "vocals"},
            )
            artifact = Path(result.artifacts["stem"])
            self.assertEqual(
                artifact.resolve().relative_to(Path(tmp).resolve()),
                Path("stem.wav"),
            )
            self.assertEqual(artifact.read_bytes(), b"test audio bytes")

        self.assertEqual(progress_values[-1], 1.0)

    def test_drums_stand_in_writes_audio_artifact(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        transform = registry.get("audio.drums_stand_in", version="1")

        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp) / "source.wav"
            source.write_bytes(b"test audio bytes")
            artifact_dir = Path(tmp) / "artifacts"
            result = transform.run(
                TransformContext(
                    artifact_dir=artifact_dir,
                    cancel_requested=lambda: False,
                    progress=lambda value: None,
                ),
                {"audio_path": str(source)},
            )

            self.assertEqual(set(result.artifacts), {"audio"})
            self.assertEqual(Path(result.artifacts["audio"]).read_bytes(), b"test audio bytes")

    def test_vocal_stand_in_label_cannot_escape_artifact_dir(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        transform = registry.get("stems.vocals_stand_in")

        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp) / "source.wav"
            source.write_bytes(b"test audio bytes")
            artifact_dir = Path(tmp) / "artifacts"
            result = transform.run(
                TransformContext(
                    artifact_dir=artifact_dir,
                    cancel_requested=lambda: False,
                    progress=lambda value: None,
                ),
                {"audio_path": str(source), "label": "../outside"},
            )
            artifact = Path(result.artifacts["stem"]).resolve()

            self.assertEqual(artifact.relative_to(artifact_dir.resolve()), Path("stem.wav"))
            self.assertFalse((Path(tmp) / "outside.wav").exists())

    def test_vocal_stand_in_cancellation_before_write_does_not_create_artifact(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        transform = registry.get("stems.vocals_stand_in")

        with tempfile.TemporaryDirectory() as tmp:
            artifact_dir = Path(tmp)
            with self.assertRaises(TransformCancelled):
                transform.run(
                    TransformContext(
                        artifact_dir=artifact_dir,
                        cancel_requested=CancelOnCall(4),
                        progress=lambda value: None,
                    ),
                    {"label": "vocals"},
                )

            self.assertFalse((artifact_dir / "stem.json").exists())


if __name__ == "__main__":
    unittest.main()
