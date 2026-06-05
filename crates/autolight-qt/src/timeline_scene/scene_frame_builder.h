#pragma once

#include "scene_snapshot_parser.h"

#include <QtCore/QRectF>
#include <QtCore/QString>
#include <QtCore/QVector>
#include <QtGui/QColor>

namespace autolight::qt::timeline_scene {

constexpr double kRulerHeight = 32.0;
constexpr double kRowHeight = 76.0;
constexpr double kLabelWidth = 280.0;
constexpr double kLeftPadding = 24.0;
constexpr double kSelectionStripeWidth = 4.0;
constexpr double kMinimumMarkerWidth = 2.0;
constexpr double kMinimumMarkerLabelWidth = 36.0;

struct RectSpec
{
  float x;
  float y;
  float width;
  float height;
};

struct BandSpec
{
  QColor color;
  QVector<RectSpec> rects;
};

struct TextSpec
{
  QString key;
  QString text;
  QColor color;
  QRectF rect;
  int pixelSize = 12;
  bool bold = false;
};

struct SceneFrameSpec
{
  QVector<BandSpec> bands;
  QVector<TextSpec> texts;
};

double timelineLaneOriginX();
double timelineLaneWidth(double width);
QRectF timelineLaneClippedRect(const QRectF& rect, double boundsWidth, double boundsHeight);
double timelineSecondsToX(double seconds, double scrollSeconds, double pixelsPerSecond);
int timelineFirstVisibleTrackIndex(double trackScrollPixels);
double timelineVisibleTrackY(int trackIndex, double trackScrollPixels);
int timelineTrackIndexForY(double y, double trackScrollPixels, const SceneSnapshot& snapshot);
QRectF timelineDisclosureRectForTrack(const TrackSpec& track, double y);
QRectF timelineMarkerRectForTrack(
  const MarkerSpec& marker,
  double y,
  double rowHeight,
  double scrollSeconds,
  double pixelsPerSecond);

SceneFrameSpec buildTimelineSceneFrame(
  const SceneSnapshot& snapshot,
  double scrollSeconds,
  double pixelsPerSecond,
  double visibleSeconds,
  double playbackPositionSeconds,
  double trackScrollPixels,
  int selectedTrackIndex,
  double width,
  double height);

} // namespace autolight::qt::timeline_scene
