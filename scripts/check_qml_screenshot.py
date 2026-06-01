import sys
from pathlib import Path

from PySide6.QtGui import QColor, QImage


TARGETS = {
    "playhead amber": QColor("#facc15"),
    "marker cyan": QColor("#67e8f9"),
}
TOOLBAR_EDGE_WIDTH = 18
TOOLBAR_HEIGHT = 40


def close_enough(left: QColor, right: QColor, tolerance: int = 14) -> bool:
    return (
        abs(left.red() - right.red()) <= tolerance
        and abs(left.green() - right.green()) <= tolerance
        and abs(left.blue() - right.blue()) <= tolerance
        and left.alpha() > 0
    )


def is_waveform_blue(color: QColor) -> bool:
    return (
        color.alpha() > 0
        and color.blue() >= 150
        and color.green() >= 100
        and color.red() <= 210
        and color.blue() >= color.red() + 20
        and color.green() >= color.red() + 10
    )


def is_clipped_toolbar_control_pixel(color: QColor) -> bool:
    return (
        color.alpha() > 0
        and color.red() < 80
        and color.green() < 80
        and color.blue() < 80
    )


def is_toolbar_background_pixel(color: QColor) -> bool:
    return (
        color.alpha() > 0
        and color.red() >= 180
        and color.green() >= 180
        and color.blue() >= 180
    )


def has_light_toolbar_region(image: QImage) -> bool:
    light_pixels = 0
    sampled_pixels = 0
    for y in range(0, min(TOOLBAR_HEIGHT, image.height()), 2):
        for x in range(0, image.width(), 4):
            sampled_pixels += 1
            if is_toolbar_background_pixel(QColor(image.pixelColor(x, y))):
                light_pixels += 1
    return sampled_pixels > 0 and light_pixels / sampled_pixels >= 0.65


def toolbar_right_edge_is_clear(image: QImage) -> bool:
    if not has_light_toolbar_region(image):
        return True
    clipped_pixels = 0
    start_x = max(0, image.width() - TOOLBAR_EDGE_WIDTH)
    for y in range(0, min(TOOLBAR_HEIGHT, image.height())):
        for x in range(start_x, image.width()):
            if is_clipped_toolbar_control_pixel(QColor(image.pixelColor(x, y))):
                clipped_pixels += 1
                if clipped_pixels >= 3:
                    return False
    return True


def count_color(image: QImage, target: QColor) -> int:
    count = 0
    for y in range(0, image.height(), 2):
        for x in range(0, image.width(), 2):
            if close_enough(QColor(image.pixelColor(x, y)), target):
                count += 1
    return count


def count_waveform_blue(image: QImage) -> int:
    count = 0
    for y in range(0, image.height(), 2):
        for x in range(0, image.width(), 2):
            if is_waveform_blue(QColor(image.pixelColor(x, y))):
                count += 1
    return count


def unique_sampled_colors(image: QImage) -> int:
    colors = set()
    for y in range(0, image.height(), 8):
        for x in range(0, image.width(), 8):
            color = QColor(image.pixelColor(x, y))
            if color.alpha() > 0:
                colors.add((color.red(), color.green(), color.blue()))
    return len(colors)


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print("usage: check_qml_screenshot.py SCREENSHOT.png", file=sys.stderr)
        return 2
    path = Path(argv[1])
    image = QImage(str(path))
    if image.isNull():
        print(f"could not load screenshot: {path}", file=sys.stderr)
        return 1
    if image.width() < 900 or image.height() < 600:
        print(f"screenshot is too small: {image.width()}x{image.height()}", file=sys.stderr)
        return 1
    if unique_sampled_colors(image) < 20:
        print("screenshot does not contain enough distinct UI colors", file=sys.stderr)
        return 1
    if not toolbar_right_edge_is_clear(image):
        print("top toolbar appears clipped at the right edge", file=sys.stderr)
        return 1
    for label, color in TARGETS.items():
        if count_color(image, color) < 4:
            print(f"missing expected {label} region", file=sys.stderr)
            return 1
    if count_waveform_blue(image) < 4:
        print("missing expected waveform blue region", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
