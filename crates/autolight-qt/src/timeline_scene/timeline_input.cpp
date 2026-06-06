#include "timeline_input.h"

#include "scene_frame_builder.h"

#include <QtCore/QPoint>
#include <QtGui/QWheelEvent>

#include <algorithm>
#include <cmath>

namespace autolight::qt::timeline_scene {

namespace {

bool isNearlyZero(double value)
{
  return std::abs(value) <= 0.000001;
}

double finiteValue(double value, double fallback)
{
  return std::isfinite(value) ? value : fallback;
}

double finiteNonNegative(double value)
{
  value = finiteValue(value, 0.0);
  return value > 0.0 ? value : 0.0;
}

} // namespace

double timelineSecondsForPosition(
  double x,
  double scrollSeconds,
  double pixelsPerSecond,
  const SceneSnapshot& snapshot)
{
  double seconds = finiteNonNegative(scrollSeconds + std::max(0.0, x - timelineLaneOriginX()) / pixelsPerSecond);
  if (snapshot.durationSeconds > 0.0) {
    seconds = std::min(seconds, snapshot.durationSeconds);
  }
  return seconds;
}

double timelineHorizontalScrollDelta(const QWheelEvent& event)
{
  const QPoint pixelDelta = event.pixelDelta();
  const QPoint angleDelta = event.angleDelta();
  const Qt::KeyboardModifiers modifiers = event.modifiers();

  double scrollDelta = -static_cast<double>(pixelDelta.x());
  if (isNearlyZero(scrollDelta)) {
    scrollDelta =
      -static_cast<double>(angleDelta.x()) / kWheelAngleUnitsPerNotch * kScrollPixelsPerNotch;
  }
  if (isNearlyZero(scrollDelta) && modifiers.testFlag(Qt::ShiftModifier)) {
    scrollDelta = -static_cast<double>(pixelDelta.y());
    if (isNearlyZero(scrollDelta)) {
      scrollDelta =
        -static_cast<double>(angleDelta.y()) / kWheelAngleUnitsPerNotch * kScrollPixelsPerNotch;
    }
  }
  return scrollDelta;
}

double timelineVerticalScrollDelta(const QWheelEvent& event)
{
  const Qt::KeyboardModifiers modifiers = event.modifiers();
  if (modifiers.testFlag(Qt::ShiftModifier)) {
    return 0.0;
  }

  const QPoint pixelDelta = event.pixelDelta();
  const QPoint angleDelta = event.angleDelta();
  double verticalDelta = -static_cast<double>(pixelDelta.y());
  if (isNearlyZero(verticalDelta)) {
    verticalDelta =
      -static_cast<double>(angleDelta.y()) / kWheelAngleUnitsPerNotch * kScrollPixelsPerNotch;
  }
  return verticalDelta;
}

double timelineZoomFactor(const QWheelEvent& event)
{
  const Qt::KeyboardModifiers modifiers = event.modifiers();
  const bool zoomGesture =
    modifiers.testFlag(Qt::ControlModifier) || modifiers.testFlag(Qt::MetaModifier);
  if (!zoomGesture) {
    return 1.0;
  }

  const QPoint pixelDelta = event.pixelDelta();
  const QPoint angleDelta = event.angleDelta();
  double zoomDelta = pixelDelta.y();
  if (isNearlyZero(zoomDelta)) {
    zoomDelta = static_cast<double>(angleDelta.y()) / kWheelAngleToDeltaFactor;
  }
  if (isNearlyZero(zoomDelta)) {
    return 1.0;
  }
  return std::pow(kZoomSensitivityBase, zoomDelta);
}

double timelineZoomAnchorX(double x)
{
  return std::max(0.0, x - timelineLaneOriginX());
}

bool timelineAdditiveSelection(Qt::KeyboardModifiers modifiers)
{
  return modifiers.testFlag(Qt::ShiftModifier);
}

} // namespace autolight::qt::timeline_scene
