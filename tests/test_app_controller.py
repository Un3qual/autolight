import unittest

from PySide6.QtCore import QCoreApplication

from autolight.app_controller import AppController


class AppControllerTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication([])

    def test_controller_loads_demo_project_into_timeline_model(self):
        controller = AppController()

        controller.load_demo_project()

        self.assertGreaterEqual(controller.trackModel.rowCount(), 2)
        self.assertEqual(controller.projectName, "Autolight Demo")


if __name__ == "__main__":
    unittest.main()
