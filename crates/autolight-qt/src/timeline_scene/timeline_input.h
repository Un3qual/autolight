#pragma once

#include "scene_snapshot_parser.h"

#include <QtCore/Qt>

class QWheelEvent;

namespace autolight::qt::timeline_scene {

constexpr double kWheelAngleToDeltaFactor = 8.0;
constexpr double kZoomSensitivityBase = 1.0015;
constexpr double kWheelAngleUnitsPerNotch = 120.0;
constexpr double kScrollPixelsPerNotch = 48.0;

double timelineSecondsForPosition(
  double x,
  double scrollSeconds,
  double pixelsPerSecond,
  const SceneSnapshot& snapshot);
double timelineHorizontalScrollDelta(const QWheelEvent& event);
double timelineVerticalScrollDelta(const QWheelEvent& event);
double timelineZoomFactor(const QWheelEvent& event);
double timelineZoomAnchorX(double x);
bool timelineAdditiveSelection(Qt::KeyboardModifiers modifiers);

} // namespace autolight::qt::timeline_scene
