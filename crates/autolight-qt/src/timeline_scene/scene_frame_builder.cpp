#include "scene_frame_builder.h"

#include <algorithm>
#include <cmath>

namespace autolight::qt::timeline_scene {

namespace {

double finiteValue(double value, double fallback)
{
  return std::isfinite(value) ? value : fallback;
}

double finiteNonNegative(double value)
{
  value = finiteValue(value, 0.0);
  return value > 0.0 ? value : 0.0;
}

bool sameDouble(double left, double right)
{
  return std::abs(left - right) <= 0.000001;
}

double treeIndentForDepth(int depth)
{
  return static_cast<double>(std::max(0, depth)) * 18.0;
}

QColor withAlpha(QColor color, int alpha)
{
  color.setAlpha(alpha);
  return color;
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
    const QRectF disclosureRect = timelineDisclosureRectForTrack(track, y);
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
  bool selected,
  double y,
  double rowHeight,
  double width,
  double height)
{
  const QColor treeGuide = withAlpha(QColor(QStringLiteral("#64748b")), 80);
  const QColor disclosureFill(selected ? QStringLiteral("#334155") : QStringLiteral("#252d39"));
  const QColor disclosureBorder(selected ? QStringLiteral("#7dd3fc") : QStringLiteral("#475569"));
  for (int depth = 1; depth <= track.depth; ++depth) {
    const double x = 21.0 + treeIndentForDepth(depth - 1);
    appendClippedRect(bands, treeGuide, x, y + 9.0, 1.0, rowHeight - 18.0, width, height);
  }
  if (!track.hasChildren) {
    return;
  }

  const QRectF disclosureRect = timelineDisclosureRectForTrack(track, y);
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
  const double endSeconds = scrollSeconds + std::max(visibleSeconds, timelineLaneWidth(width) / pixelsPerSecond);
  double tickSeconds = std::floor(scrollSeconds / minorStepSeconds) * minorStepSeconds;

  int tickCount = 0;
  while (tickSeconds <= endSeconds && tickCount < 1000) {
    const double ratio = tickSeconds / majorStepSeconds;
    const bool major = sameDouble(ratio, std::round(ratio));
    const double x = originX + timelineSecondsToX(tickSeconds, scrollSeconds, pixelsPerSecond);
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
  const double endSeconds = scrollSeconds + std::max(visibleSeconds, timelineLaneWidth(width) / pixelsPerSecond);
  double tickSeconds = std::floor(scrollSeconds / minorStepSeconds) * minorStepSeconds;

  int tickCount = 0;
  while (tickSeconds <= endSeconds && tickCount < 1000) {
    const double ratio = tickSeconds / majorStepSeconds;
    const bool major = sameDouble(ratio, std::round(ratio));
    const double tickHeight = major ? 18.0 : 9.0;
    const double tickWidth = major ? 1.5 : 1.0;
    const double x = originX + timelineSecondsToX(tickSeconds, scrollSeconds, pixelsPerSecond);
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
    const double sampleX = timelineSecondsToX(sample.time, scrollSeconds, pixelsPerSecond);
    const double nextX = timelineSecondsToX(nextTime, scrollSeconds, pixelsPerSecond);
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

} // namespace

double timelineLaneOriginX()
{
  return kLabelWidth + kLeftPadding;
}

double timelineLaneWidth(double width)
{
  return std::max(0.0, width - timelineLaneOriginX());
}

double timelineSecondsToX(double seconds, double scrollSeconds, double pixelsPerSecond)
{
  return (seconds - scrollSeconds) * pixelsPerSecond;
}

int timelineFirstVisibleTrackIndex(double trackScrollPixels)
{
  return std::max(0, static_cast<int>(std::floor(finiteNonNegative(trackScrollPixels) / kRowHeight)));
}

double timelineVisibleTrackY(int trackIndex, double trackScrollPixels)
{
  return kRulerHeight
    + static_cast<double>(trackIndex - timelineFirstVisibleTrackIndex(trackScrollPixels)) * kRowHeight;
}

int timelineTrackIndexForY(double y, double trackScrollPixels, const SceneSnapshot& snapshot)
{
  if (y < kRulerHeight) {
    return -1;
  }
  const int visibleRow = static_cast<int>(std::floor((y - kRulerHeight) / kRowHeight));
  const int trackIndex = timelineFirstVisibleTrackIndex(trackScrollPixels) + visibleRow;
  return trackIndex >= 0 && trackIndex < snapshot.tracks.size() ? trackIndex : -1;
}

QRectF timelineDisclosureRectForTrack(const TrackSpec& track, double y)
{
  return QRectF(12.0 + treeIndentForDepth(track.depth), y + 9.0, 20.0, 20.0);
}

QRectF timelineMarkerRectForTrack(
  const MarkerSpec& marker,
  double y,
  double rowHeight,
  double scrollSeconds,
  double pixelsPerSecond)
{
  const double markerX = timelineLaneOriginX()
    + timelineSecondsToX(marker.timestamp, scrollSeconds, pixelsPerSecond);
  const double markerWidth = std::max(kMinimumMarkerWidth, marker.duration * pixelsPerSecond);
  return QRectF(
    markerX,
    y + (marker.selected ? 7.0 : 10.0),
    markerWidth,
    rowHeight - (marker.selected ? 14.0 : 20.0));
}

SceneFrameSpec buildTimelineSceneFrame(
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
  const double originX = timelineLaneOriginX();

  appendClippedRect(bands, pageBackground, 0.0, 0.0, width, height, width, height);
  appendClippedRect(bands, rulerBackground, 0.0, 0.0, width, kRulerHeight, width, height);
  appendClippedRect(bands, labelBackground, 0.0, 0.0, kLabelWidth, kRulerHeight, width, height);
  appendClippedRect(bands, rulerEdge, kLabelWidth - 1.0, 0.0, 1.0, height, width, height);
  appendRulerTicks(bands, scrollSeconds, pixelsPerSecond, visibleSeconds, originX, width, height);
  appendClippedRect(bands, rulerEdge, 0.0, kRulerHeight - 1.0, width, 1.0, width, height);
  appendText(texts, QStringLiteral("TRACKS"), QColor(QStringLiteral("#94a3b8")), 14.0, 6.0, kLabelWidth - 28.0, 20.0, 10, true);

  const int selectedIndex = resolvedSelectedTrackIndex(snapshot, selectedTrackIndex);
  const int firstTrackIndex = timelineFirstVisibleTrackIndex(trackScrollPixels);
  for (int trackIndex = firstTrackIndex; trackIndex < snapshot.tracks.size(); ++trackIndex) {
    const double y = timelineVisibleTrackY(trackIndex, trackScrollPixels);
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
    appendTrackTreeChrome(bands, track, selected, y, rowHeight, width, height);
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
      const double sampleX = timelineSecondsToX(sample.time, scrollSeconds, pixelsPerSecond);
      const double nextX = timelineSecondsToX(nextTime, scrollSeconds, pixelsPerSecond);
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
      const QRectF markerRect = timelineMarkerRectForTrack(marker, y, rowHeight, scrollSeconds, pixelsPerSecond);
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

  const double playheadX = originX
    + timelineSecondsToX(playbackPositionSeconds, scrollSeconds, pixelsPerSecond);
  appendClippedRect(bands, withAlpha(playhead, 245), playheadX - 1.0, 0.0, 2.0, height, width, height);
  appendClippedRect(bands, withAlpha(playhead, 245), playheadX - 5.0, 0.0, 10.0, 5.0, width, height);

  return frame;
}

} // namespace autolight::qt::timeline_scene
