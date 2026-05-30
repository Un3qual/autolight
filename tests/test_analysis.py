import json
import tempfile
import unittest
from pathlib import Path

from autolight.analysis import TransformCancelled
from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformContext, TransformRegistry


class CancelOnCall:
    def __init__(self, call_number):
        self.call_number = call_number
        self.calls = 0

    def __call__(self):
        self.calls += 1
        return self.calls >= self.call_number


class AnalysisRegistryTest(unittest.TestCase):
    def test_builtin_registry_contains_marker_and_expensive_transforms(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        self.assertIn("markers.fixed_interval", registry.ids())
        self.assertIn("stems.vocals_stand_in", registry.ids())

    def test_registry_get_supports_version_lookup(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        transform = registry.get("markers.fixed_interval", version="1")

        self.assertEqual(transform.version, "1")
        with self.assertRaises(ValueError):
            registry.get("markers.fixed_interval", version="2")

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

    def test_vocal_stand_in_writes_artifact(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        transform = registry.get("stems.vocals_stand_in")
        progress_values = []

        with tempfile.TemporaryDirectory() as tmp:
            result = transform.run(
                TransformContext(
                    artifact_dir=Path(tmp),
                    cancel_requested=lambda: False,
                    progress=progress_values.append,
                ),
                {"label": "vocals"},
            )
            artifact = Path(result.artifacts["stem"])
            self.assertEqual(
                artifact.resolve().relative_to(Path(tmp).resolve()),
                Path("stem.json"),
            )
            self.assertEqual(
                json.loads(artifact.read_text(encoding="utf-8")),
                {"samples": [], "stem": "vocals"},
            )

        self.assertEqual(progress_values[-1], 1.0)

    def test_vocal_stand_in_label_cannot_escape_artifact_dir(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        transform = registry.get("stems.vocals_stand_in")

        with tempfile.TemporaryDirectory() as tmp:
            artifact_dir = Path(tmp) / "artifacts"
            result = transform.run(
                TransformContext(
                    artifact_dir=artifact_dir,
                    cancel_requested=lambda: False,
                    progress=lambda value: None,
                ),
                {"label": "../outside"},
            )
            artifact = Path(result.artifacts["stem"]).resolve()

            self.assertEqual(artifact.relative_to(artifact_dir.resolve()), Path("stem.json"))
            self.assertFalse((Path(tmp) / "outside.json").exists())

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
