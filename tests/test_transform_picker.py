import unittest

from PySide6.QtCore import QCoreApplication

from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformRegistry, TransformResult, TransformSpec
from autolight.project.models import Track, TrackType
from autolight.timeline.transform_model import TransformSpecModel


class TransformPickerTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication([])

    def test_transform_model_exposes_registered_specs(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        model = TransformSpecModel(registry)

        ids = [
            model.data(model.index(row, 0), model.role_for_name("transformId"))
            for row in range(model.rowCount())
        ]

        self.assertIn("markers.fixed_interval", ids)
        self.assertIn("stems.vocals_stand_in", ids)

    def test_transform_model_exposes_multiple_versions_for_same_transform(self):
        def noop(context, params):
            return TransformResult()

        registry = TransformRegistry()
        registry.register(
            TransformSpec(
                id="test.versioned",
                version="1",
                name="Versioned 1",
                input_schema="audio.v1",
                output_schema="markers.v1",
                estimated_cost="light",
                run=noop,
            )
        )
        registry.register(
            TransformSpec(
                id="test.versioned",
                version="2",
                name="Versioned 2",
                input_schema="audio.v1",
                output_schema="markers.v1",
                estimated_cost="light",
                run=noop,
            )
        )

        model = TransformSpecModel(registry)
        versions = [
            model.data(model.index(row, 0), model.role_for_name("version"))
            for row in range(model.rowCount())
        ]

        self.assertEqual(versions, ["1", "2"])


if __name__ == "__main__":
    unittest.main()
