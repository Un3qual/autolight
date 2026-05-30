import sys

from PySide6.QtGui import QGuiApplication
from PySide6.QtQml import QQmlApplicationEngine

from autolight.app_controller import AppController


def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv if argv is None else argv)
    app = QGuiApplication(args)
    controller = AppController()
    controller.load_demo_project()

    engine = QQmlApplicationEngine()
    engine.rootContext().setContextProperty("appController", controller)
    engine.addImportPath(sys.path[0])
    engine.loadFromModule("UI", "Main")
    if not engine.rootObjects():
        return -1
    if "--smoke" in args:
        del engine
        return 0
    exit_code = app.exec()
    del engine
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
