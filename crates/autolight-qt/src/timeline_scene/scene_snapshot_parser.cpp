#include "scene_snapshot_parser.h"

#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QJsonValue>

#include <algorithm>
#include <cmath>

namespace autolight::qt::timeline_scene {

namespace {

double finiteValue(double value, double fallback)
{
  return std::isfinite(value) ? value : fallback;
}

double finiteJsonNumber(const QJsonValue& value, double fallback)
{
  return finiteValue(value.toDouble(fallback), fallback);
}

double finiteNonNegative(double value)
{
  value = finiteValue(value, 0.0);
  return value > 0.0 ? value : 0.0;
}

QColor parseColor(const QString& value, const QColor& fallback)
{
  const QColor color(value);
  return color.isValid() ? color : fallback;
}

} // namespace

SceneSnapshot parseTimelineSceneSnapshot(const QString& sceneSnapshotJson)
{
  SceneSnapshot snapshot;
  if (sceneSnapshotJson.trimmed().isEmpty()) {
    return snapshot;
  }

  const QJsonDocument document = QJsonDocument::fromJson(sceneSnapshotJson.toUtf8());
  if (!document.isObject()) {
    return snapshot;
  }

  const QJsonObject root = document.object();
  snapshot.durationSeconds = finiteNonNegative(
    finiteJsonNumber(root.value(QStringLiteral("durationSeconds")), 0.0));

  const QJsonArray tracks = root.value(QStringLiteral("tracks")).toArray();
  snapshot.tracks.reserve(tracks.size());
  for (const QJsonValue& trackValue : tracks) {
    if (!trackValue.isObject()) {
      continue;
    }
    const QJsonObject trackObject = trackValue.toObject();
    const QString trackId = trackObject.value(QStringLiteral("trackId")).toString().trimmed();
    if (trackId.isEmpty()) {
      continue;
    }
    TrackSpec track;
    track.trackId = trackId;
    track.name = trackObject.value(QStringLiteral("name")).toString(track.trackId);
    track.trackType = trackObject.value(QStringLiteral("trackType")).toString();
    track.resultState = trackObject.value(QStringLiteral("resultState")).toString();
    track.depth = std::max(0, trackObject.value(QStringLiteral("depth")).toInt(0));
    track.hasChildren = trackObject.value(QStringLiteral("hasChildren")).toBool(false);
    track.selected = trackObject.value(QStringLiteral("selected")).toBool(false);
    track.expanded = trackObject.value(QStringLiteral("expanded")).toBool(false);

    const QJsonArray markers = trackObject.value(QStringLiteral("markers")).toArray();
    track.markers.reserve(markers.size());
    for (const QJsonValue& markerValue : markers) {
      const QJsonObject markerObject = markerValue.toObject();
      MarkerSpec marker;
      marker.markerId = markerObject.value(QStringLiteral("markerId")).toString();
      marker.timestamp = finiteNonNegative(
        finiteJsonNumber(markerObject.value(QStringLiteral("timestamp")), 0.0));
      marker.duration = finiteNonNegative(
        finiteJsonNumber(markerObject.value(QStringLiteral("duration")), 0.0));
      marker.color = parseColor(
        markerObject.value(QStringLiteral("color")).toString(QStringLiteral("#f59e0b")),
        QColor(QStringLiteral("#f59e0b")));
      marker.selected = markerObject.value(QStringLiteral("selected")).toBool(false);
      track.markers.push_back(marker);
    }

    const QJsonArray waveformPreview = trackObject.value(QStringLiteral("waveformPreview")).toArray();
    track.waveformPreview.reserve(waveformPreview.size());
    for (const QJsonValue& sampleValue : waveformPreview) {
      const QJsonObject sampleObject = sampleValue.toObject();
      WaveformSampleSpec sample;
      sample.time = finiteNonNegative(
        finiteJsonNumber(sampleObject.value(QStringLiteral("time")), 0.0));
      sample.peak = std::clamp(
        finiteJsonNumber(sampleObject.value(QStringLiteral("peak")), 0.0), 0.0, 1.0);
      sample.rms = std::clamp(
        finiteJsonNumber(sampleObject.value(QStringLiteral("rms")), 0.0), 0.0, 1.0);
      track.waveformPreview.push_back(sample);
    }

    const QJsonArray analysisPreviews = trackObject.value(QStringLiteral("analysisPreviews")).toArray();
    track.analysisPreviews.reserve(analysisPreviews.size());
    for (const QJsonValue& previewValue : analysisPreviews) {
      const QJsonObject previewObject = previewValue.toObject();
      AnalysisPreviewSpec preview;
      preview.artifactKind = previewObject.value(QStringLiteral("artifactKind")).toString();
      const QJsonArray samples = previewObject.value(QStringLiteral("samples")).toArray();
      preview.samples.reserve(samples.size());
      for (const QJsonValue& sampleValue : samples) {
        const QJsonObject sampleObject = sampleValue.toObject();
        AnalysisSampleSpec sample;
        sample.time = finiteNonNegative(
          finiteJsonNumber(sampleObject.value(QStringLiteral("time")), 0.0));
        sample.intensity = std::clamp(
          finiteJsonNumber(sampleObject.value(QStringLiteral("intensity")), 1.0), 0.0, 1.0);
        sample.color = parseColor(
          sampleObject.value(QStringLiteral("color")).toString(QStringLiteral("#93c5fd")),
          QColor(QStringLiteral("#93c5fd")));
        preview.samples.push_back(sample);
      }
      track.analysisPreviews.push_back(preview);
    }
    snapshot.tracks.push_back(track);
  }

  return snapshot;
}

} // namespace autolight::qt::timeline_scene
