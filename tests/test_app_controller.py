import unittest
from pathlib import Path
from unittest.mock import patch

from PySide6.QtCore import QCoreApplication

import main as app_entry
from autolight.app_controller import AppController


class FakeContext:
    def __init__(self):
        self.properties = {}

    def setContextProperty(self, name, value):
        self.properties[name] = value


class FakeEngine:
    instances = []
    root_objects = [object()]

    def __init__(self):
        self.context = FakeContext()
        self.import_paths = []
        self.loaded_modules = []
        type(self).instances.append(self)

    def rootContext(self):
        return self.context

    def addImportPath(self, path):
        self.import_paths.append(path)

    def loadFromModule(self, module, name):
        self.loaded_modules.append((module, name))

    def rootObjects(self):
        return type(self).root_objects


class FakeGuiApplication:
    def __init__(self, args):
        self.args = args
        self.exec_called = False

    def exec(self):
        self.exec_called = True
        raise AssertionError("smoke mode must not enter the Qt event loop")


class AppControllerTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication([])

    def test_controller_loads_demo_project_into_timeline_model(self):
        controller = AppController()

        controller.load_demo_project()

        self.assertEqual(controller.trackModel.rowCount(), 3)
        self.assertEqual(controller.projectName, "Autolight Demo")

    def test_controller_emits_project_name_changed_when_demo_loads(self):
        controller = AppController()
        project_names = []
        controller.projectNameChanged.connect(lambda: project_names.append(controller.projectName))

        controller.load_demo_project()

        self.assertEqual(project_names, ["Autolight Demo"])

    def test_controller_parents_track_model(self):
        controller = AppController()

        self.assertIs(controller.trackModel.parent(), controller)

    def test_controller_demo_project_exposes_expected_track_roles(self):
        controller = AppController()

        controller.load_demo_project()

        self.assertEqual(
            [self._track_role_values(controller, row) for row in range(controller.trackModel.rowCount())],
            [
                {
                    "name_prefix": "autolight-demo-",
                    "trackType": "source",
                    "resultState": "complete",
                    "markerCount": 0,
                },
                {
                    "name": "Beat Markers",
                    "trackType": "generated",
                    "resultState": "complete",
                    "markerCount": 3,
                },
                {
                    "name": "Editable Cues",
                    "trackType": "editable",
                    "resultState": "complete",
                    "markerCount": 0,
                },
            ],
        )

    def test_controller_uses_unique_demo_audio_paths(self):
        controller = AppController()

        controller.load_demo_project()
        first_path = controller._project.audio_assets[0].path
        controller.load_demo_project()
        second_path = controller._project.audio_assets[0].path

        self.assertNotEqual(first_path, second_path)

    def test_smoke_loads_qml_before_returning(self):
        FakeEngine.instances = []
        FakeEngine.root_objects = [object()]

        with patch.object(app_entry, "QGuiApplication", FakeGuiApplication), patch.object(
            app_entry,
            "QQmlApplicationEngine",
            FakeEngine,
        ):
            exit_code = app_entry.main(["main.py", "--smoke"])

        self.assertEqual(exit_code, 0)
        self.assertEqual(len(FakeEngine.instances), 1)
        self.assertEqual(FakeEngine.instances[0].loaded_modules, [("UI", "Main")])
        self.assertIsInstance(FakeEngine.instances[0].context.properties["appController"], AppController)

    def test_smoke_fails_when_qml_root_does_not_load(self):
        FakeEngine.instances = []
        FakeEngine.root_objects = []

        with patch.object(app_entry, "QGuiApplication", FakeGuiApplication), patch.object(
            app_entry,
            "QQmlApplicationEngine",
            FakeEngine,
        ):
            exit_code = app_entry.main(["main.py", "--smoke"])

        self.assertEqual(exit_code, -1)

    def test_qml_timeline_shell_uses_one_row_oriented_list(self):
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")

        self.assertEqual(qml.count("ListView {"), 1)
        self.assertIn("id: timelineRows", qml)
        self.assertNotIn("onContentYChanged", qml)
        self.assertNotIn("contentY =", qml)

    def _track_role_values(self, controller: AppController, row: int):
        model = controller.trackModel
        index = model.index(row, 0)
        values = {
            "trackType": model.data(index, model.role_for_name("trackType")),
            "resultState": model.data(index, model.role_for_name("resultState")),
            "markerCount": model.data(index, model.role_for_name("markerCount")),
        }
        name = model.data(index, model.role_for_name("name"))
        if values["trackType"] == "source":
            values["name_prefix"] = name[: len("autolight-demo-")]
        else:
            values["name"] = name
        return values


if __name__ == "__main__":
    unittest.main()
