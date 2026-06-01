import sys

from PySide6.QtCore import QTimer
from PySide6.QtGui import QGuiApplication
from PySide6.QtQml import QQmlApplicationEngine

from autolight.app_controller import AppController


def _argument_value(args: list[str], flag: str) -> str:
    try:
        return args[args.index(flag) + 1]
    except (ValueError, IndexError):
        return ""


def _grab_root_image(root):
    if hasattr(root, "grabWindow"):
        return root.grabWindow()
    screen = root.screen() or QGuiApplication.primaryScreen()
    if screen is None:
        raise RuntimeError("no screen is available for screenshot capture")
    return screen.grabWindow(root.winId()).toImage()


def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv if argv is None else argv)
    app = QGuiApplication(args)
    controller = AppController()
    controller.load_demo_project()

    engine = QQmlApplicationEngine()
    engine.rootContext().setContextProperty("appController", controller)
    engine.addImportPath(sys.path[0])
    engine.loadFromModule("UI", "Main")
    try:
        if not engine.rootObjects():
            return -1
        screenshot_path = _argument_value(args, "--screenshot")
        if screenshot_path:
            controller.play_selected_track()
            controller.pause_playback()
            controller.seek_playback(0.35)
            root = engine.rootObjects()[0]

            def capture() -> None:
                try:
                    image = _grab_root_image(root)
                except Exception as exc:
                    print(f"could not capture screenshot: {exc}", file=sys.stderr)
                    app.exit(2)
                    return
                if not image.save(screenshot_path):
                    app.exit(2)
                    return
                app.exit(0)

            QTimer.singleShot(150, capture)
            return app.exec()
        if "--smoke" in args:
            return 0
        return app.exec()
    finally:
        del engine
        controller.cleanup()


if __name__ == "__main__":
    sys.exit(main())
