#include "timeline_scene_item.h"

#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QRectF>
#include <QtCore/QSize>
#include <QtCore/QVector>
#include <QtGui/QColor>
#include <QtGui/QFont>
#include <QtGui/QFontMetrics>
#include <QtGui/QImage>
#include <QtGui/QMouseEvent>
#include <QtGui/QPainter>
#include <QtGui/QWheelEvent>
#include <QtQuick/QQuickWindow>
#include <QtQuick/QSGFlatColorMaterial>
#include <QtQuick/QSGGeometry>
#include <QtQuick/QSGGeometryNode>
#include <QtQuick/QSGSimpleTextureNode>

#include <algorithm>
#include <cmath>
#include <memory>

namespace {

constexpr double kRulerHeight = 32.0;
constexpr double kRowHeight = 76.0;
constexpr double kLabelWidth = 280.0;
constexpr double kLeftPadding = 24.0;
constexpr double kSelectionStripeWidth = 4.0;
constexpr double kMinimumMarkerWidth = 2.0;
constexpr double kWheelAngleToDeltaFactor = 8.0;
constexpr double kZoomSensitivityBase = 1.0015;
constexpr double kWheelAngleUnitsPerNotch = 120.0;
constexpr double kScrollPixelsPerNotch = 48.0;

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

double finitePositive(double value, double fallback)
{
  value = finiteValue(value, fallback);
  return value > 0.0 ? value : fallback;
}

bool sameDouble(double left, double right)
{
  return std::abs(left - right) <= 0.000001;
}

double laneOriginX()
{
  return kLabelWidth + kLeftPadding;
}

double laneWidth(double width)
{
  return std::max(0.0, width - laneOriginX());
}

double treeIndentForDepth(int depth)
{
  return static_cast<double>(std::max(0, depth)) * 18.0;
}

QRectF disclosureRectForTrack(const TrackSpec& track, double y)
{
  return QRectF(12.0 + treeIndentForDepth(track.depth), y + 9.0, 20.0, 20.0);
}

int firstVisibleTrackIndex(double trackScrollPixels)
{
  return std::max(0, static_cast<int>(std::floor(finiteNonNegative(trackScrollPixels) / kRowHeight)));
}

double visibleTrackY(int trackIndex, double trackScrollPixels)
{
  return kRulerHeight + static_cast<double>(trackIndex - firstVisibleTrackIndex(trackScrollPixels)) * kRowHeight;
}

QColor parseColor(const QString& value, const QColor& fallback)
{
  const QColor color(value);
  return color.isValid() ? color : fallback;
}

QColor withAlpha(QColor color, int alpha)
{
  color.setAlpha(alpha);
  return color;
}

SceneSnapshot parseSnapshot(const QString& sceneSnapshotJson)
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
    const QJsonObject trackObject = trackValue.toObject();
    TrackSpec track;
    track.trackId = trackObject.value(QStringLiteral("trackId")).toString();
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

void addRectVertices(QSGGeometry::Point2D* vertices, int offset, const RectSpec& rect)
{
  const float left = rect.x;
  const float right = rect.x + rect.width;
  const float top = rect.y;
  const float bottom = rect.y + rect.height;

  vertices[offset].set(left, top);
  vertices[offset + 1].set(right, top);
  vertices[offset + 2].set(left, bottom);
  vertices[offset + 3].set(right, top);
  vertices[offset + 4].set(right, bottom);
  vertices[offset + 5].set(left, bottom);
}

void appendRect(QVector<BandSpec>& bands, const QColor& color, const RectSpec& rect)
{
  if (rect.width <= 0.0F || rect.height <= 0.0F) {
    return;
  }
  if (!bands.isEmpty() && bands.last().color == color) {
    bands.last().rects.push_back(rect);
    return;
  }
  BandSpec band;
  band.color = color;
  band.rects.push_back(rect);
  bands.push_back(band);
}

void appendClippedRect(
  QVector<BandSpec>& bands,
  const QColor& color,
  double x,
  double y,
  double width,
  double height,
  double boundsWidth,
  double boundsHeight)
{
  const double left = std::max(0.0, x);
  const double top = std::max(0.0, y);
  const double right = std::min(boundsWidth, x + width);
  const double bottom = std::min(boundsHeight, y + height);
  if (right <= left || bottom <= top) {
    return;
  }
  appendRect(
    bands,
    color,
    RectSpec{
      static_cast<float>(left),
      static_cast<float>(top),
      static_cast<float>(right - left),
      static_cast<float>(bottom - top),
    });
}

void appendText(
  QVector<TextSpec>& texts,
  const QString& text,
  const QColor& color,
  double x,
  double y,
  double width,
  double height,
  int pixelSize,
  bool bold)
{
  if (text.trimmed().isEmpty() || width <= 0.0 || height <= 0.0) {
    return;
  }
  TextSpec spec;
  spec.text = text;
  spec.color = color;
  spec.rect = QRectF(x, y, width, height);
  spec.pixelSize = pixelSize;
  spec.bold = bold;
  spec.key = QStringLiteral("%1|%2|%3|%4|%5x%6")
               .arg(text, color.name(QColor::HexArgb))
               .arg(pixelSize)
               .arg(bold ? 1 : 0)
               .arg(std::lround(width))
               .arg(std::lround(height));
  texts.push_back(spec);
}

void appendTrackLabel(
  QVector<TextSpec>& texts,
  const TrackSpec& track,
  double y,
  double rowHeight,
  bool selected)
{
  const QColor titleColor = selected
    ? QColor(QStringLiteral("#f8fafc"))
    : QColor(QStringLiteral("#e5e7eb"));
  const QColor metaColor = selected
    ? QColor(QStringLiteral("#bae6fd"))
    : QColor(QStringLiteral("#94a3b8"));
  const QString title = track.name.isEmpty() ? track.trackId : track.name;
  QString meta;
  if (!track.trackType.isEmpty()) {
    meta += track.trackType.toUpper();
  }
  if (!track.resultState.isEmpty()) {
    if (!meta.isEmpty()) {
      meta += QStringLiteral("  ");
    }
    meta += track.resultState.toUpper();
  }

  const double indent = treeIndentForDepth(track.depth);
  const double disclosureWidth = track.hasChildren ? 26.0 : 0.0;
  const double titleX = 14.0 + indent + disclosureWidth;
  if (track.hasChildren) {
    const QRectF disclosureRect = disclosureRectForTrack(track, y);
    appendText(
      texts,
      track.expanded ? QStringLiteral("v") : QStringLiteral(">"),
      metaColor,
      disclosureRect.x(),
      disclosureRect.y(),
      disclosureRect.width(),
      disclosureRect.height(),
      13,
      true);
  }
  appendText(
    texts,
    title,
    titleColor,
    titleX,
    y + 9.0,
    kLabelWidth - titleX - 14.0,
    22.0,
    14,
    selected);
  appendText(
    texts,
    meta,
    metaColor,
    titleX,
    y + rowHeight - 31.0,
    kLabelWidth - titleX - 14.0,
    18.0,
    11,
    false);
}

void appendTrackTreeChrome(
  QVector<BandSpec>& bands,
  const TrackSpec& track,
  double y,
  double rowHeight,
  double width,
  double height)
{
  const QColor treeGuide = withAlpha(QColor(QStringLiteral("#64748b")), 80);
  const QColor disclosureFill(track.selected ? QStringLiteral("#334155") : QStringLiteral("#252d39"));
  const QColor disclosureBorder(track.selected ? QStringLiteral("#7dd3fc") : QStringLiteral("#475569"));
  for (int depth = 1; depth <= track.depth; ++depth) {
    const double x = 21.0 + treeIndentForDepth(depth - 1);
    appendClippedRect(bands, treeGuide, x, y + 9.0, 1.0, rowHeight - 18.0, width, height);
  }
  if (!track.hasChildren) {
    return;
  }

  const QRectF disclosureRect = disclosureRectForTrack(track, y);
  appendClippedRect(
    bands,
    disclosureFill,
    disclosureRect.x(),
    disclosureRect.y(),
    disclosureRect.width(),
    disclosureRect.height(),
    width,
    height);
  appendClippedRect(
    bands,
    disclosureBorder,
    disclosureRect.x(),
    disclosureRect.y(),
    disclosureRect.width(),
    1.0,
    width,
    height);
  appendClippedRect(
    bands,
    disclosureBorder,
    disclosureRect.x(),
    disclosureRect.y() + disclosureRect.height() - 1.0,
    disclosureRect.width(),
    1.0,
    width,
    height);
  appendClippedRect(
    bands,
    disclosureBorder,
    disclosureRect.x(),
    disclosureRect.y(),
    1.0,
    disclosureRect.height(),
    width,
    height);
  appendClippedRect(
    bands,
    disclosureBorder,
    disclosureRect.x() + disclosureRect.width() - 1.0,
    disclosureRect.y(),
    1.0,
    disclosureRect.height(),
    width,
    height);
}

void appendTrackLaneChrome(
  QVector<BandSpec>& bands,
  bool selected,
  int trackIndex,
  double y,
  double rowHeight,
  double originX,
  double width,
  double height)
{
  const QColor laneGutter(trackIndex % 2 == 0 ? QStringLiteral("#18202a") : QStringLiteral("#151c25"));
  const QColor laneRowBackground = selected
    ? QColor(QStringLiteral("#1d3344"))
    : QColor(trackIndex % 2 == 0 ? QStringLiteral("#1a2330") : QStringLiteral("#17202b"));
  const QColor laneRowBorder = selected
    ? QColor(QStringLiteral("#38bdf8"))
    : QColor(QStringLiteral("#334155"));
  const QColor laneCenterGuide = selected
    ? withAlpha(QColor(QStringLiteral("#7dd3fc")), 85)
    : withAlpha(QColor(QStringLiteral("#64748b")), 45);
  const double chromeTop = y + 7.0;
  const double chromeHeight = std::max(1.0, rowHeight - 14.0);
  const double chromeWidth = std::max(0.0, width - originX);
  const double centerY = chromeTop + chromeHeight / 2.0;

  appendClippedRect(bands, laneGutter, kLabelWidth, y, originX - kLabelWidth, rowHeight, width, height);
  appendClippedRect(
    bands,
    selected ? withAlpha(laneRowBackground, 245) : laneRowBackground,
    originX,
    chromeTop,
    chromeWidth,
    chromeHeight,
    width,
    height);
  appendClippedRect(
    bands,
    selected ? withAlpha(laneRowBorder, 220) : withAlpha(laneRowBorder, 135),
    originX,
    chromeTop,
    chromeWidth,
    selected ? 1.5 : 1.0,
    width,
    height);
  appendClippedRect(
    bands,
    selected ? withAlpha(laneRowBorder, 220) : withAlpha(laneRowBorder, 125),
    originX,
    chromeTop + chromeHeight - 1.0,
    chromeWidth,
    selected ? 1.5 : 1.0,
    width,
    height);
  appendClippedRect(
    bands,
    selected ? withAlpha(laneRowBorder, 220) : withAlpha(laneRowBorder, 115),
    originX,
    chromeTop,
    1.0,
    chromeHeight,
    width,
    height);
  appendClippedRect(bands, laneCenterGuide, originX, centerY, chromeWidth, 1.0, width, height);
}

int resolvedSelectedTrackIndex(const SceneSnapshot& snapshot, int requestedIndex)
{
  if (requestedIndex >= 0 && requestedIndex < snapshot.tracks.size()) {
    return requestedIndex;
  }
  for (int index = 0; index < snapshot.tracks.size(); ++index) {
    if (snapshot.tracks[index].selected) {
      return index;
    }
  }
  return -1;
}

double niceRulerStep(double rawSeconds)
{
  if (!std::isfinite(rawSeconds) || rawSeconds <= 0.0) {
    return 1.0;
  }

  const double magnitude = std::pow(10.0, std::floor(std::log10(rawSeconds)));
  const double normalized = rawSeconds / magnitude;
  if (normalized <= 1.0) {
    return magnitude;
  }
  if (normalized <= 2.0) {
    return 2.0 * magnitude;
  }
  if (normalized <= 5.0) {
    return 5.0 * magnitude;
  }
  return 10.0 * magnitude;
}

double secondsToX(double seconds, double scrollSeconds, double pixelsPerSecond)
{
  return (seconds - scrollSeconds) * pixelsPerSecond;
}

double secondsForPosition(
  double x,
  double scrollSeconds,
  double pixelsPerSecond,
  const SceneSnapshot& snapshot)
{
  double seconds = finiteNonNegative(scrollSeconds + std::max(0.0, x - laneOriginX()) / pixelsPerSecond);
  if (snapshot.durationSeconds > 0.0) {
    seconds = std::min(seconds, snapshot.durationSeconds);
  }
  return seconds;
}

bool additiveSelection(const QMouseEvent& event)
{
  return event.modifiers().testFlag(Qt::ShiftModifier);
}

QRectF markerRectForTrack(
  const MarkerSpec& marker,
  double y,
  double rowHeight,
  double scrollSeconds,
  double pixelsPerSecond)
{
  const double markerX = laneOriginX() + secondsToX(marker.timestamp, scrollSeconds, pixelsPerSecond);
  const double markerWidth = std::max(kMinimumMarkerWidth, marker.duration * pixelsPerSecond);
  return QRectF(
    markerX,
    y + (marker.selected ? 7.0 : 10.0),
    markerWidth,
    rowHeight - (marker.selected ? 14.0 : 20.0));
}

int trackIndexForY(double y, double trackScrollPixels, const SceneSnapshot& snapshot)
{
  if (y < kRulerHeight) {
    return -1;
  }
  const int visibleRow = static_cast<int>(std::floor((y - kRulerHeight) / kRowHeight));
  const int trackIndex = firstVisibleTrackIndex(trackScrollPixels) + visibleRow;
  return trackIndex >= 0 && trackIndex < snapshot.tracks.size() ? trackIndex : -1;
}

void appendTimelineGridLines(
  QVector<BandSpec>& bands,
  double scrollSeconds,
  double pixelsPerSecond,
  double visibleSeconds,
  double originX,
  double y,
  double rowHeight,
  double width,
  double height)
{
  const QColor minorGrid = withAlpha(QColor(QStringLiteral("#475569")), 38);
  const QColor majorGrid = withAlpha(QColor(QStringLiteral("#94a3b8")), 58);
  const double majorStepSeconds = niceRulerStep(120.0 / pixelsPerSecond);
  const double minorStepSeconds = majorStepSeconds / 4.0;
  const double endSeconds = scrollSeconds + std::max(visibleSeconds, laneWidth(width) / pixelsPerSecond);
  double tickSeconds = std::floor(scrollSeconds / minorStepSeconds) * minorStepSeconds;

  int tickCount = 0;
  while (tickSeconds <= endSeconds && tickCount < 1000) {
    const double ratio = tickSeconds / majorStepSeconds;
    const bool major = sameDouble(ratio, std::round(ratio));
    const double x = originX + secondsToX(tickSeconds, scrollSeconds, pixelsPerSecond);
    appendClippedRect(
      bands,
      major ? majorGrid : minorGrid,
      x,
      y,
      major ? 1.25 : 1.0,
      rowHeight,
      width,
      height);
    tickSeconds += minorStepSeconds;
    ++tickCount;
  }
}

void appendRulerTicks(
  QVector<BandSpec>& bands,
  double scrollSeconds,
  double pixelsPerSecond,
  double visibleSeconds,
  double originX,
  double width,
  double height)
{
  const QColor minorTick(QStringLiteral("#2d3540"));
  const QColor majorTick(QStringLiteral("#64748b"));
  const double majorStepSeconds = niceRulerStep(120.0 / pixelsPerSecond);
  const double minorStepSeconds = majorStepSeconds / 4.0;
  const double endSeconds = scrollSeconds + std::max(visibleSeconds, laneWidth(width) / pixelsPerSecond);
  double tickSeconds = std::floor(scrollSeconds / minorStepSeconds) * minorStepSeconds;

  int tickCount = 0;
  while (tickSeconds <= endSeconds && tickCount < 1000) {
    const double ratio = tickSeconds / majorStepSeconds;
    const bool major = sameDouble(ratio, std::round(ratio));
    const double tickHeight = major ? 18.0 : 9.0;
    const double tickWidth = major ? 1.5 : 1.0;
    const double x = originX + secondsToX(tickSeconds, scrollSeconds, pixelsPerSecond);
    appendClippedRect(
      bands,
      major ? majorTick : minorTick,
      x,
      kRulerHeight - tickHeight,
      tickWidth,
      tickHeight,
      width,
      height);
    tickSeconds += minorStepSeconds;
    ++tickCount;
  }
}

void appendAnalysisPreview(
  QVector<BandSpec>& bands,
  const AnalysisPreviewSpec& preview,
  double scrollSeconds,
  double pixelsPerSecond,
  double y,
  double rowHeight,
  double originX,
  double width,
  double height)
{
  if (preview.samples.isEmpty()) {
    return;
  }
  const bool energy = preview.artifactKind == QStringLiteral("energy");
  const double stripHeight = energy ? 18.0 : 14.0;
  const double stripBottomMargin = energy ? 14.0 : 4.0;
  const double stripTop = y + rowHeight - stripBottomMargin - stripHeight;
  for (int sampleIndex = 0; sampleIndex < preview.samples.size(); ++sampleIndex) {
    const AnalysisSampleSpec& sample = preview.samples[sampleIndex];
    const double nextTime = sampleIndex + 1 < preview.samples.size()
      ? preview.samples[sampleIndex + 1].time
      : sample.time + 0.05;
    const double sampleX = secondsToX(sample.time, scrollSeconds, pixelsPerSecond);
    const double nextX = secondsToX(nextTime, scrollSeconds, pixelsPerSecond);
    const double sampleWidth = std::max(1.0, nextX - sampleX);
    const double sampleHeight = energy ? std::max(1.0, sample.intensity * stripHeight) : stripHeight;
    const QColor sampleColor = energy ? QColor(QStringLiteral("#facc15")) : sample.color;
    appendClippedRect(
      bands,
      withAlpha(sampleColor, energy ? 170 : 150),
      originX + sampleX,
      stripTop + (stripHeight - sampleHeight),
      sampleWidth,
      sampleHeight,
      width,
      height);
  }
}

SceneFrameSpec buildSceneFrame(
  const SceneSnapshot& snapshot,
  double scrollSeconds,
  double pixelsPerSecond,
  double visibleSeconds,
  double playbackPositionSeconds,
  double trackScrollPixels,
  int selectedTrackIndex,
  double width,
  double height)
{
  SceneFrameSpec frame;
  QVector<BandSpec>& bands = frame.bands;
  QVector<TextSpec>& texts = frame.texts;
  const QColor pageBackground(QStringLiteral("#0f1318"));
  const QColor rulerBackground(QStringLiteral("#171c23"));
  const QColor labelBackground(QStringLiteral("#1c222b"));
  const QColor selectedLabelBackground(QStringLiteral("#263241"));
  const QColor rulerEdge(QStringLiteral("#2f3742"));
  const QColor laneEven(QStringLiteral("#141920"));
  const QColor laneOdd(QStringLiteral("#11161c"));
  const QColor laneDivider(QStringLiteral("#232b35"));
  const QColor selectionStripe(QStringLiteral("#67e8f9"));
  const QColor selectionOutline(QStringLiteral("#38bdf8"));
  const QColor playhead(QStringLiteral("#f43f5e"));
  const double originX = laneOriginX();

  appendClippedRect(bands, pageBackground, 0.0, 0.0, width, height, width, height);
  appendClippedRect(bands, rulerBackground, 0.0, 0.0, width, kRulerHeight, width, height);
  appendClippedRect(bands, labelBackground, 0.0, 0.0, kLabelWidth, kRulerHeight, width, height);
  appendClippedRect(bands, rulerEdge, kLabelWidth - 1.0, 0.0, 1.0, height, width, height);
  appendRulerTicks(bands, scrollSeconds, pixelsPerSecond, visibleSeconds, originX, width, height);
  appendClippedRect(bands, rulerEdge, 0.0, kRulerHeight - 1.0, width, 1.0, width, height);
  appendText(texts, QStringLiteral("TRACKS"), QColor(QStringLiteral("#94a3b8")), 14.0, 6.0, kLabelWidth - 28.0, 20.0, 10, true);

  const int selectedIndex = resolvedSelectedTrackIndex(snapshot, selectedTrackIndex);
  const int firstTrackIndex = firstVisibleTrackIndex(trackScrollPixels);
  for (int trackIndex = firstTrackIndex; trackIndex < snapshot.tracks.size(); ++trackIndex) {
    const double y = visibleTrackY(trackIndex, trackScrollPixels);
    if (y >= height) {
      break;
    }
    const double rowHeight = std::min(kRowHeight, height - y);
    const QColor laneColor = trackIndex % 2 == 0 ? laneEven : laneOdd;
    const bool selected = trackIndex == selectedIndex;
    appendClippedRect(
      bands,
      selected ? selectedLabelBackground : labelBackground,
      0.0,
      y,
      kLabelWidth,
      rowHeight,
      width,
      height);
    appendClippedRect(bands, laneColor, kLabelWidth, y, width - kLabelWidth, rowHeight, width, height);
    appendClippedRect(bands, laneDivider, 0.0, y + rowHeight - 1.0, width, 1.0, width, height);
    appendClippedRect(bands, rulerEdge, kLabelWidth - 1.0, y, 1.0, rowHeight, width, height);
    appendTrackLaneChrome(bands, selected, trackIndex, y, rowHeight, originX, width, height);
    appendTimelineGridLines(
      bands,
      scrollSeconds,
      pixelsPerSecond,
      visibleSeconds,
      originX,
      y + 7.0,
      std::max(1.0, rowHeight - 14.0),
      width,
      height);

    if (selected) {
      appendClippedRect(
        bands, withAlpha(selectionStripe, 230), 0.0, y, kSelectionStripeWidth, rowHeight, width, height);
      appendClippedRect(bands, withAlpha(selectionOutline, 180), 0.0, y, width, 1.0, width, height);
      appendClippedRect(
        bands, withAlpha(selectionOutline, 180), 0.0, y + rowHeight - 1.0, width, 1.0, width, height);
      appendClippedRect(bands, withAlpha(selectionOutline, 160), width - 1.0, y, 1.0, rowHeight, width, height);
    }

    const TrackSpec& track = snapshot.tracks[trackIndex];
    appendTrackTreeChrome(bands, track, y, rowHeight, width, height);
    appendTrackLabel(texts, track, y, rowHeight, selected);

    const double waveformTop = y + 12.0;
    const double waveformHeight = std::max(1.0, rowHeight - 24.0);
    const double waveformCenterY = waveformTop + waveformHeight / 2.0;
    const double waveformScaleY = waveformHeight / 2.0;
    for (int sampleIndex = 0; sampleIndex < track.waveformPreview.size(); ++sampleIndex) {
      const WaveformSampleSpec& sample = track.waveformPreview[sampleIndex];
      const double nextTime = sampleIndex + 1 < track.waveformPreview.size()
        ? track.waveformPreview[sampleIndex + 1].time
        : sample.time + 0.01;
      const double sampleX = secondsToX(sample.time, scrollSeconds, pixelsPerSecond);
      const double nextX = secondsToX(nextTime, scrollSeconds, pixelsPerSecond);
      const double sampleWidth = std::max(1.0, nextX - sampleX);
      const double peakHeight = std::max(1.0, sample.peak * waveformScaleY);
      const double rmsHeight = std::max(1.0, sample.rms * waveformScaleY);
      appendClippedRect(
        bands,
        QColor(QStringLiteral("#1d4ed8")),
        originX + sampleX,
        waveformCenterY - peakHeight,
        sampleWidth,
        peakHeight * 2.0,
        width,
        height);
      appendClippedRect(
        bands,
        QColor(QStringLiteral("#60a5fa")),
        originX + sampleX,
        waveformCenterY - rmsHeight,
        sampleWidth,
        rmsHeight * 2.0,
        width,
        height);
    }
    for (const AnalysisPreviewSpec& preview : track.analysisPreviews) {
      appendAnalysisPreview(
        bands,
        preview,
        scrollSeconds,
        pixelsPerSecond,
        y,
        rowHeight,
        originX,
        width,
        height);
    }
    for (const MarkerSpec& marker : track.markers) {
      const QRectF markerRect = markerRectForTrack(marker, y, rowHeight, scrollSeconds, pixelsPerSecond);
      QColor markerFill = marker.color;
      markerFill.setAlpha(marker.selected ? 230 : 155);
      appendClippedRect(
        bands,
        markerFill,
        markerRect.x(),
        markerRect.y(),
        markerRect.width(),
        markerRect.height(),
        width,
        height);
      appendClippedRect(
        bands,
        withAlpha(marker.color, marker.selected ? 230 : 140),
        markerRect.x(),
        kRulerHeight - 7.0,
        std::max(1.5, std::min(markerRect.width(), 5.0)),
        6.0,
        width,
        height);
    }
  }

  const double playheadX = originX + secondsToX(playbackPositionSeconds, scrollSeconds, pixelsPerSecond);
  appendClippedRect(bands, withAlpha(playhead, 245), playheadX - 1.0, 0.0, 2.0, height, width, height);
  appendClippedRect(bands, withAlpha(playhead, 245), playheadX - 5.0, 0.0, 10.0, 5.0, width, height);

  return frame;
}

int childCount(QSGNode* root)
{
  int count = 0;
  for (QSGNode* child = root->firstChild(); child != nullptr; child = child->nextSibling()) {
    ++count;
  }
  return count;
}

QSGNode* childNodeAt(QSGNode* root, int targetIndex)
{
  int index = 0;
  for (QSGNode* child = root->firstChild(); child != nullptr; child = child->nextSibling()) {
    if (index == targetIndex) {
      return child;
    }
    ++index;
  }
  return nullptr;
}

QSGNode* ensureContainerNode(QSGNode* root, int index)
{
  if (QSGNode* existing = childNodeAt(root, index)) {
    return existing;
  }
  while (childCount(root) <= index) {
    root->appendChildNode(new QSGNode());
  }
  return childNodeAt(root, index);
}

QSGGeometryNode* geometryChildAt(QSGNode* root, int targetIndex)
{
  return static_cast<QSGGeometryNode*>(childNodeAt(root, targetIndex));
}

QSGGeometryNode* createBandNode()
{
  auto* node = new QSGGeometryNode();
  auto* geometry = new QSGGeometry(QSGGeometry::defaultAttributes_Point2D(), 0);
  geometry->setDrawingMode(QSGGeometry::DrawTriangles);
  node->setGeometry(geometry);
  node->setFlag(QSGNode::OwnsGeometry);
  auto* material = new QSGFlatColorMaterial();
  node->setMaterial(material);
  node->setFlag(QSGNode::OwnsMaterial);
  return node;
}

QSGGeometryNode* ensureBandNode(QSGNode* root, int index)
{
  if (auto* existing = geometryChildAt(root, index)) {
    return existing;
  }
  auto* node = createBandNode();
  root->appendChildNode(node);
  return node;
}

void trimChildNodes(QSGNode* root, int targetCount)
{
  while (childCount(root) > targetCount) {
    QSGNode* child = root->lastChild();
    root->removeChildNode(child);
    delete child;
  }
}

void updateBandNode(QSGGeometryNode* node, const BandSpec& band)
{
  auto* geometry = node->geometry();
  const int vertexCount = static_cast<int>(band.rects.size() * 6);
  if (geometry == nullptr) {
    geometry = new QSGGeometry(QSGGeometry::defaultAttributes_Point2D(), vertexCount);
    geometry->setDrawingMode(QSGGeometry::DrawTriangles);
    node->setGeometry(geometry);
    node->setFlag(QSGNode::OwnsGeometry);
  } else if (geometry->vertexCount() != vertexCount) {
    geometry->allocate(vertexCount);
  }

  auto* vertices = geometry->vertexDataAsPoint2D();
  for (qsizetype index = 0; index < band.rects.size(); ++index) {
    addRectVertices(vertices, static_cast<int>(index) * 6, band.rects[index]);
  }

  auto* material = static_cast<QSGFlatColorMaterial*>(node->material());
  if (material == nullptr) {
    material = new QSGFlatColorMaterial();
    node->setMaterial(material);
    node->setFlag(QSGNode::OwnsMaterial);
  }
  material->setColor(band.color);

  node->markDirty(QSGNode::DirtyGeometry | QSGNode::DirtyMaterial);
}

class TextTextureNode : public QSGSimpleTextureNode
{
public:
  QString key;
};

TextTextureNode* textChildAt(QSGNode* root, int targetIndex)
{
  return static_cast<TextTextureNode*>(childNodeAt(root, targetIndex));
}

TextTextureNode* ensureTextNode(QSGNode* root, int index)
{
  if (auto* existing = textChildAt(root, index)) {
    return existing;
  }
  auto* node = new TextTextureNode();
  node->setOwnsTexture(true);
  root->appendChildNode(node);
  return node;
}

void updateTextNode(TextTextureNode* node, const TextSpec& spec, QQuickWindow* window)
{
  if (node == nullptr || window == nullptr) {
    return;
  }

  node->setRect(spec.rect);
  const double devicePixelRatio = std::max(1.0, window->effectiveDevicePixelRatio());
  const QSize imageSize(
    std::max(1, static_cast<int>(std::ceil(spec.rect.width() * devicePixelRatio))),
    std::max(1, static_cast<int>(std::ceil(spec.rect.height() * devicePixelRatio))));
  const QString key = QStringLiteral("%1|%2|%3").arg(spec.key).arg(imageSize.width()).arg(imageSize.height());
  if (node->key == key) {
    return;
  }

  QImage image(imageSize, QImage::Format_ARGB32_Premultiplied);
  image.setDevicePixelRatio(devicePixelRatio);
  image.fill(Qt::transparent);

  QFont font;
  font.setPixelSize(spec.pixelSize);
  font.setBold(spec.bold);

  QPainter painter(&image);
  painter.setRenderHint(QPainter::TextAntialiasing, true);
  painter.setFont(font);
  painter.setPen(spec.color);
  const QFontMetrics metrics(font);
  const QString elided = metrics.elidedText(spec.text, Qt::ElideRight, static_cast<int>(spec.rect.width()));
  painter.drawText(QRectF(0.0, 0.0, spec.rect.width(), spec.rect.height()), Qt::AlignLeft | Qt::AlignVCenter, elided);
  painter.end();

  node->setTexture(window->createTextureFromImage(image));
  node->setOwnsTexture(true);
  node->key = key;
  node->markDirty(QSGNode::DirtyGeometry | QSGNode::DirtyMaterial);
}

void updateTextNodes(QSGNode* root, const QVector<TextSpec>& texts, QQuickWindow* window)
{
  if (window == nullptr) {
    trimChildNodes(root, 0);
    return;
  }
  for (int index = 0; index < texts.size(); ++index) {
    updateTextNode(ensureTextNode(root, index), texts[index], window);
  }
  trimChildNodes(root, texts.size());
}

QSGNode* updateRootNode(QSGNode* root, const SceneFrameSpec& frame, QQuickWindow* window)
{
  QSGNode* geometryRoot = ensureContainerNode(root, 0);
  QSGNode* textRoot = ensureContainerNode(root, 1);

  for (int index = 0; index < frame.bands.size(); ++index) {
    updateBandNode(ensureBandNode(geometryRoot, index), frame.bands[index]);
  }
  trimChildNodes(geometryRoot, frame.bands.size());
  updateTextNodes(textRoot, frame.texts, window);
  trimChildNodes(root, 2);
  return root;
}

} // namespace

struct TimelineSceneSnapshotData
{
  SceneSnapshot snapshot;
};

TimelineSceneItem::TimelineSceneItem(QQuickItem* parent)
  : QQuickItem(parent)
  , m_snapshot(std::make_unique<TimelineSceneSnapshotData>())
{
  setFlag(QQuickItem::ItemHasContents, true);
  setAcceptedMouseButtons(Qt::LeftButton);
  setAcceptHoverEvents(true);
}

TimelineSceneItem::~TimelineSceneItem() = default;

QString TimelineSceneItem::sceneSnapshotJson() const
{
  return m_sceneSnapshotJson;
}

void TimelineSceneItem::setSceneSnapshotJson(const QString& sceneSnapshotJson)
{
  if (m_sceneSnapshotJson == sceneSnapshotJson) {
    return;
  }
  m_sceneSnapshotJson = sceneSnapshotJson;
  if (m_snapshot == nullptr) {
    m_snapshot = std::make_unique<TimelineSceneSnapshotData>();
  }
  m_snapshot->snapshot = parseSnapshot(m_sceneSnapshotJson);
  update();
  emit sceneSnapshotJsonChanged();
}

double TimelineSceneItem::viewportScrollSeconds() const
{
  return m_viewportScrollSeconds;
}

void TimelineSceneItem::setViewportScrollSeconds(double viewportScrollSeconds)
{
  viewportScrollSeconds = finiteNonNegative(viewportScrollSeconds);
  if (sameDouble(m_viewportScrollSeconds, viewportScrollSeconds)) {
    return;
  }
  m_viewportScrollSeconds = viewportScrollSeconds;
  update();
  emit viewportScrollSecondsChanged();
}

double TimelineSceneItem::viewportPixelsPerSecond() const
{
  return m_viewportPixelsPerSecond;
}

void TimelineSceneItem::setViewportPixelsPerSecond(double viewportPixelsPerSecond)
{
  viewportPixelsPerSecond = finitePositive(viewportPixelsPerSecond, 100.0);
  if (sameDouble(m_viewportPixelsPerSecond, viewportPixelsPerSecond)) {
    return;
  }
  m_viewportPixelsPerSecond = viewportPixelsPerSecond;
  update();
  emit viewportPixelsPerSecondChanged();
}

double TimelineSceneItem::viewportVisibleSeconds() const
{
  return m_viewportVisibleSeconds;
}

void TimelineSceneItem::setViewportVisibleSeconds(double viewportVisibleSeconds)
{
  viewportVisibleSeconds = finitePositive(viewportVisibleSeconds, 1.0);
  if (sameDouble(m_viewportVisibleSeconds, viewportVisibleSeconds)) {
    return;
  }
  m_viewportVisibleSeconds = viewportVisibleSeconds;
  update();
  emit viewportVisibleSecondsChanged();
}

double TimelineSceneItem::viewportTrackScrollPixels() const
{
  return m_viewportTrackScrollPixels;
}

void TimelineSceneItem::setViewportTrackScrollPixels(double viewportTrackScrollPixels)
{
  viewportTrackScrollPixels = finiteNonNegative(viewportTrackScrollPixels);
  if (sameDouble(m_viewportTrackScrollPixels, viewportTrackScrollPixels)) {
    return;
  }
  m_viewportTrackScrollPixels = viewportTrackScrollPixels;
  update();
  emit viewportTrackScrollPixelsChanged();
}

double TimelineSceneItem::playbackPositionSeconds() const
{
  return m_playbackPositionSeconds;
}

void TimelineSceneItem::setPlaybackPositionSeconds(double playbackPositionSeconds)
{
  playbackPositionSeconds = finiteNonNegative(playbackPositionSeconds);
  if (sameDouble(m_playbackPositionSeconds, playbackPositionSeconds)) {
    return;
  }
  m_playbackPositionSeconds = playbackPositionSeconds;
  update();
  emit playbackPositionSecondsChanged();
}

int TimelineSceneItem::selectedTrackIndex() const
{
  return m_selectedTrackIndex;
}

void TimelineSceneItem::setSelectedTrackIndex(int selectedTrackIndex)
{
  if (m_selectedTrackIndex == selectedTrackIndex) {
    return;
  }
  m_selectedTrackIndex = selectedTrackIndex;
  update();
  emit selectedTrackIndexChanged();
}

QSGNode* TimelineSceneItem::updatePaintNode(QSGNode* oldNode, UpdatePaintNodeData*)
{
  const double itemWidth = width();
  const double itemHeight = height();
  if (itemWidth <= 0.0 || itemHeight <= 0.0) {
    delete oldNode;
    return nullptr;
  }

  const SceneSnapshot emptySnapshot;
  const SceneSnapshot& snapshot = m_snapshot != nullptr ? m_snapshot->snapshot : emptySnapshot;
  const double pixelsPerSecond = finitePositive(m_viewportPixelsPerSecond, 100.0);
  const double scrollSeconds = finiteNonNegative(m_viewportScrollSeconds);
  const double visibleSeconds = finitePositive(m_viewportVisibleSeconds, itemWidth / pixelsPerSecond);
  const double playbackPositionSeconds = finiteNonNegative(m_playbackPositionSeconds);
  const SceneFrameSpec frame = buildSceneFrame(
    snapshot,
    scrollSeconds,
    pixelsPerSecond,
    visibleSeconds,
    playbackPositionSeconds,
    m_viewportTrackScrollPixels,
    m_selectedTrackIndex,
    itemWidth,
    itemHeight);

  QSGNode* root = oldNode != nullptr ? oldNode : new QSGNode();
  return updateRootNode(root, frame, window());
}

void TimelineSceneItem::mousePressEvent(QMouseEvent* event)
{
  if (event == nullptr) {
    return;
  }

  const double x = event->position().x();
  const double y = event->position().y();
  const double pixelsPerSecond = finitePositive(m_viewportPixelsPerSecond, 100.0);
  const double scrollSeconds = finiteNonNegative(m_viewportScrollSeconds);
  const double trackScrollPixels = finiteNonNegative(m_viewportTrackScrollPixels);
  const SceneSnapshot emptySnapshot;
  const SceneSnapshot& snapshot = m_snapshot != nullptr ? m_snapshot->snapshot : emptySnapshot;

  if (y < kRulerHeight) {
    const double seconds = secondsForPosition(x, scrollSeconds, pixelsPerSecond, snapshot);
    m_scrubbingRuler = true;
    emit scrubRequested(seconds);
    event->accept();
    return;
  }

  const int trackIndex = trackIndexForY(y, trackScrollPixels, snapshot);
  if (trackIndex >= 0) {
    const TrackSpec& track = snapshot.tracks[trackIndex];
    const QString& trackId = track.trackId;
    if (!trackId.isEmpty()) {
      const double rowY = visibleTrackY(trackIndex, trackScrollPixels);
      const double rowHeight = std::min(kRowHeight, height() - rowY);
      for (int markerIndex = static_cast<int>(track.markers.size()) - 1; markerIndex >= 0; --markerIndex) {
        const MarkerSpec& marker = track.markers[markerIndex];
        if (!marker.markerId.isEmpty()
            && markerRectForTrack(marker, rowY, rowHeight, scrollSeconds, pixelsPerSecond).contains(event->position())) {
          emit markerClicked(trackId, marker.markerId, additiveSelection(*event));
          event->accept();
          return;
        }
      }
      const QRectF disclosureRect = disclosureRectForTrack(track, rowY);
      if (track.hasChildren && disclosureRect.contains(event->position())) {
        emit trackExpansionToggled(trackId, !track.expanded);
        event->accept();
        return;
      }
      emit trackClicked(trackId);
      if (x >= laneOriginX()) {
        emit scrubRequested(secondsForPosition(x, scrollSeconds, pixelsPerSecond, snapshot));
      }
      event->accept();
      return;
    }
  }

  event->ignore();
}

void TimelineSceneItem::mouseMoveEvent(QMouseEvent* event)
{
  if (event == nullptr || !m_scrubbingRuler) {
    return;
  }
  const SceneSnapshot emptySnapshot;
  const SceneSnapshot& snapshot = m_snapshot != nullptr ? m_snapshot->snapshot : emptySnapshot;
  emit scrubRequested(secondsForPosition(
    event->position().x(),
    finiteNonNegative(m_viewportScrollSeconds),
    finitePositive(m_viewportPixelsPerSecond, 100.0),
    snapshot));
  event->accept();
}

void TimelineSceneItem::mouseReleaseEvent(QMouseEvent* event)
{
  if (event == nullptr || !m_scrubbingRuler) {
    return;
  }
  const SceneSnapshot emptySnapshot;
  const SceneSnapshot& snapshot = m_snapshot != nullptr ? m_snapshot->snapshot : emptySnapshot;
  emit scrubRequested(secondsForPosition(
    event->position().x(),
    finiteNonNegative(m_viewportScrollSeconds),
    finitePositive(m_viewportPixelsPerSecond, 100.0),
    snapshot));
  m_scrubbingRuler = false;
  event->accept();
}

void TimelineSceneItem::wheelEvent(QWheelEvent* event)
{
  if (event == nullptr) {
    return;
  }

  const QPoint pixelDelta = event->pixelDelta();
  const QPoint angleDelta = event->angleDelta();
  const Qt::KeyboardModifiers modifiers = event->modifiers();
  const bool zoomGesture =
    modifiers.testFlag(Qt::ControlModifier) || modifiers.testFlag(Qt::MetaModifier);

  if (zoomGesture) {
    double zoomDelta = pixelDelta.y();
    if (std::abs(zoomDelta) <= 0.000001) {
      zoomDelta = static_cast<double>(angleDelta.y()) / kWheelAngleToDeltaFactor;
    }
    if (std::abs(zoomDelta) > 0.000001) {
      const double factor = std::pow(kZoomSensitivityBase, zoomDelta);
      emit viewportZoomRequested(factor, std::max(0.0, event->position().x() - laneOriginX()));
      event->accept();
      return;
    }
  }

  double scrollDelta = -static_cast<double>(pixelDelta.x());
  if (std::abs(scrollDelta) <= 0.000001) {
    scrollDelta =
      -static_cast<double>(angleDelta.x()) / kWheelAngleUnitsPerNotch * kScrollPixelsPerNotch;
  }
  if (std::abs(scrollDelta) <= 0.000001 && modifiers.testFlag(Qt::ShiftModifier)) {
    scrollDelta = -static_cast<double>(pixelDelta.y());
    if (std::abs(scrollDelta) <= 0.000001) {
      scrollDelta =
        -static_cast<double>(angleDelta.y()) / kWheelAngleUnitsPerNotch * kScrollPixelsPerNotch;
    }
  }
  if (std::abs(scrollDelta) > 0.000001) {
    emit viewportScrollRequested(scrollDelta);
    event->accept();
    return;
  }

  if (!modifiers.testFlag(Qt::ShiftModifier)) {
    double verticalDelta = -static_cast<double>(pixelDelta.y());
    if (std::abs(verticalDelta) <= 0.000001) {
      verticalDelta =
        -static_cast<double>(angleDelta.y()) / kWheelAngleUnitsPerNotch * kScrollPixelsPerNotch;
    }
    if (std::abs(verticalDelta) > 0.000001) {
      emit viewportVerticalScrollRequested(verticalDelta);
      event->accept();
      return;
    }
  }

  event->ignore();
}
