#pragma once

#include <QtCore/QString>
#include <QtCore/QVector>
#include <QtGui/QColor>

namespace autolight::qt::timeline_scene {

struct MarkerSpec
{
  QString markerId;
  double timestamp = 0.0;
  double duration = 0.0;
  QColor color = QColor(QStringLiteral("#f59e0b"));
  bool selected = false;
};

struct WaveformSampleSpec
{
  double time = 0.0;
  double peak = 0.0;
  double rms = 0.0;
};

struct AnalysisSampleSpec
{
  double time = 0.0;
  double intensity = 1.0;
  QColor color = QColor(QStringLiteral("#93c5fd"));
};

struct AnalysisPreviewSpec
{
  QString artifactKind;
  QVector<AnalysisSampleSpec> samples;
};

struct TrackSpec
{
  QString trackId;
  QString name;
  QString trackType;
  QString resultState;
  int depth = 0;
  bool hasChildren = false;
  bool selected = false;
  bool expanded = false;
  QVector<MarkerSpec> markers;
  QVector<WaveformSampleSpec> waveformPreview;
  QVector<AnalysisPreviewSpec> analysisPreviews;
};

struct SceneSnapshot
{
  QVector<TrackSpec> tracks;
  double durationSeconds = 0.0;
};

struct TimelineSceneSnapshotData
{
  SceneSnapshot snapshot;
};

SceneSnapshot parseTimelineSceneSnapshot(const QString& sceneSnapshotJson);

} // namespace autolight::qt::timeline_scene
