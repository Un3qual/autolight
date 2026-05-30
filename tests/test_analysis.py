import tempfile
import unittest
from pathlib import Path

from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformContext, TransformRegistry


class AnalysisRegistryTest(unittest.TestCase):
    def test_builtin_registry_contains_marker_and_expensive_transforms(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        self.assertIn("markers.fixed_interval", registry.ids())
        self.assertIn("stems.vocals_stand_in", registry.ids())

    def test_fixed_interval_transform_returns_markers(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        transform = registry.get("markers.fixed_interval")

        with tempfile.TemporaryDirectory() as tmp:
            result = transform.run(
                TransformContext(artifact_dir=Path(tmp), cancel_requested=lambda: False, progress=lambda value: None),
                {"duration": 2.0, "interval": 0.5},
            )

        self.assertEqual([marker["timestamp"] for marker in result.markers], [0.0, 0.5, 1.0, 1.5, 2.0])
        self.assertEqual(result.artifacts, {})

    def test_vocal_stand_in_writes_artifact(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        transform = registry.get("stems.vocals_stand_in")
        progress_values = []

        with tempfile.TemporaryDirectory() as tmp:
            result = transform.run(
                TransformContext(artifact_dir=Path(tmp), cancel_requested=lambda: False, progress=progress_values.append),
                {"label": "vocals"},
            )
            self.assertTrue(Path(result.artifacts["stem"]).exists())

        self.assertEqual(progress_values[-1], 1.0)


if __name__ == "__main__":
    unittest.main()
