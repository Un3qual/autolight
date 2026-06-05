#include "timeline_scene_item.h"

#include "scene_frame_builder.h"
#include "scene_graph_nodes.h"
#include "scene_snapshot_parser.h"
#include "timeline_input.h"

#include <QtCore/QElapsedTimer>
#include <QtCore/QMetaObject>
#include <QtGui/QMouseEvent>
#include <QtGui/QWheelEvent>
#include <QtQuick/QSGNode>

#include <algorithm>
#include <atomic>
#include <cmath>
#include <memory>

using namespace autolight::qt::timeline_scene;

namespace {

double itemFiniteValue(double value, double fallback)
{
  return std::isfinite(value) ? value : fallback;
}

double itemFiniteNonNegative(double value)
{
  value = itemFiniteValue(value, 0.0);
  return value > 0.0 ? value : 0.0;
}

double itemFinitePositive(double value, double fallback)
{
  value = itemFiniteValue(value, fallback);
  return value > 0.0 ? value : fallback;
}

bool itemSameDouble(double left, double right)
{
  return std::abs(left - right) <= 0.000001;
}

qulonglong elapsedMicros(const QElapsedTimer& timer)
{
  const qint64 elapsedNanos = timer.nsecsElapsed();
  if (elapsedNanos <= 0) {
    return 0;
  }
  return static_cast<qulonglong>((elapsedNanos + 999) / 1000);
}

bool updateAtomicMax(std::atomic<qulonglong>& value, qulonglong candidate)
{
  qulonglong current = value.load(std::memory_order_relaxed);
  while (candidate > current) {
    if (value.compare_exchange_weak(
          current, candidate, std::memory_order_relaxed, std::memory_order_relaxed)) {
      return true;
    }
  }
  return false;
}

const SceneSnapshot& currentSnapshot(
  const std::unique_ptr<TimelineSceneSnapshotData>& snapshot,
  const SceneSnapshot& fallback)
{
  return snapshot != nullptr ? snapshot->snapshot : fallback;
}

bool hasMeaningfulDelta(double value)
{
  return std::abs(value) > 0.000001;
}

} // namespace

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
  QElapsedTimer parseTimer;
  parseTimer.start();
  m_snapshot->snapshot = parseTimelineSceneSnapshot(m_sceneSnapshotJson);
  const qulonglong parseMicros = elapsedMicros(parseTimer);
  ++m_sceneSnapshotParseCount;
  if (parseMicros > m_worstSceneSnapshotParseMicros) {
    m_worstSceneSnapshotParseMicros = parseMicros;
  }
  update();
  emit sceneSnapshotJsonChanged();
  emit scenePerfCountersChanged();
}

double TimelineSceneItem::viewportScrollSeconds() const
{
  return m_viewportScrollSeconds;
}

void TimelineSceneItem::setViewportScrollSeconds(double viewportScrollSeconds)
{
  viewportScrollSeconds = itemFiniteNonNegative(viewportScrollSeconds);
  if (itemSameDouble(m_viewportScrollSeconds, viewportScrollSeconds)) {
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
  viewportPixelsPerSecond = itemFinitePositive(viewportPixelsPerSecond, 100.0);
  if (itemSameDouble(m_viewportPixelsPerSecond, viewportPixelsPerSecond)) {
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
  viewportVisibleSeconds = itemFinitePositive(viewportVisibleSeconds, 1.0);
  if (itemSameDouble(m_viewportVisibleSeconds, viewportVisibleSeconds)) {
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
  viewportTrackScrollPixels = itemFiniteNonNegative(viewportTrackScrollPixels);
  if (itemSameDouble(m_viewportTrackScrollPixels, viewportTrackScrollPixels)) {
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
  playbackPositionSeconds = itemFiniteNonNegative(playbackPositionSeconds);
  if (itemSameDouble(m_playbackPositionSeconds, playbackPositionSeconds)) {
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

qulonglong TimelineSceneItem::sceneSnapshotParseCount() const
{
  return m_sceneSnapshotParseCount;
}

qulonglong TimelineSceneItem::worstSceneSnapshotParseMicros() const
{
  return m_worstSceneSnapshotParseMicros;
}

qulonglong TimelineSceneItem::worstSceneGraphUpdateMicros() const
{
  return m_worstSceneGraphUpdateMicros.load(std::memory_order_relaxed);
}

qulonglong TimelineSceneItem::textTextureCreateCount() const
{
  return m_textTextureCreateCount.load(std::memory_order_relaxed);
}

void TimelineSceneItem::queueScenePerfCountersChanged()
{
  bool expected = false;
  if (!m_scenePerfCountersNotifyQueued.compare_exchange_strong(
        expected, true, std::memory_order_acq_rel, std::memory_order_acquire)) {
    return;
  }

  const bool queued = QMetaObject::invokeMethod(
    this,
    [this]() {
      m_scenePerfCountersNotifyQueued.store(false, std::memory_order_release);
      emit scenePerfCountersChanged();
    },
    Qt::QueuedConnection);
  if (!queued) {
    m_scenePerfCountersNotifyQueued.store(false, std::memory_order_release);
  }
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
  const SceneSnapshot& snapshot = currentSnapshot(m_snapshot, emptySnapshot);
  const double pixelsPerSecond = itemFinitePositive(m_viewportPixelsPerSecond, 100.0);
  const double scrollSeconds = itemFiniteNonNegative(m_viewportScrollSeconds);
  const double visibleSeconds = itemFinitePositive(m_viewportVisibleSeconds, itemWidth / pixelsPerSecond);
  const double playbackPositionSeconds = itemFiniteNonNegative(m_playbackPositionSeconds);
  const SceneFrameSpec frame = buildTimelineSceneFrame(
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
  SceneGraphUpdateStats stats;
  QElapsedTimer graphTimer;
  graphTimer.start();
  QSGNode* updatedRoot = updateTimelineSceneGraph(root, frame, window(), &stats);
  const qulonglong graphMicros = elapsedMicros(graphTimer);
  bool perfCountersChanged = updateAtomicMax(m_worstSceneGraphUpdateMicros, graphMicros);
  if (stats.textTexturesCreated > 0) {
    m_textTextureCreateCount.fetch_add(stats.textTexturesCreated, std::memory_order_relaxed);
    perfCountersChanged = true;
  }
  if (perfCountersChanged) {
    queueScenePerfCountersChanged();
  }
  return updatedRoot;
}

void TimelineSceneItem::mousePressEvent(QMouseEvent* event)
{
  if (event == nullptr) {
    return;
  }

  const double x = event->position().x();
  const double y = event->position().y();
  const double pixelsPerSecond = itemFinitePositive(m_viewportPixelsPerSecond, 100.0);
  const double scrollSeconds = itemFiniteNonNegative(m_viewportScrollSeconds);
  const double trackScrollPixels = itemFiniteNonNegative(m_viewportTrackScrollPixels);
  const SceneSnapshot emptySnapshot;
  const SceneSnapshot& snapshot = currentSnapshot(m_snapshot, emptySnapshot);

  if (y < kRulerHeight) {
    m_scrubbingRuler = true;
    emit scrubRequested(timelineSecondsForPosition(x, scrollSeconds, pixelsPerSecond, snapshot));
    event->accept();
    return;
  }

  const int trackIndex = timelineTrackIndexForY(y, trackScrollPixels, snapshot);
  if (trackIndex < 0) {
    event->ignore();
    return;
  }

  const TrackSpec& track = snapshot.tracks[trackIndex];
  const QString& trackId = track.trackId;
  if (trackId.isEmpty()) {
    event->ignore();
    return;
  }

  const double rowY = timelineVisibleTrackY(trackIndex, trackScrollPixels);
  const double rowHeight = std::min(kRowHeight, height() - rowY);
  for (int markerIndex = static_cast<int>(track.markers.size()) - 1; markerIndex >= 0; --markerIndex) {
    const MarkerSpec& marker = track.markers[markerIndex];
    if (!marker.markerId.isEmpty()
        && timelineMarkerRectForTrack(marker, rowY, rowHeight, scrollSeconds, pixelsPerSecond)
             .contains(event->position())) {
      emit markerClicked(trackId, marker.markerId, timelineAdditiveSelection(event->modifiers()));
      event->accept();
      return;
    }
  }

  const QRectF disclosureRect = timelineDisclosureRectForTrack(track, rowY);
  if (track.hasChildren && disclosureRect.contains(event->position())) {
    emit trackExpansionToggled(trackId, !track.expanded);
    event->accept();
    return;
  }

  emit trackClicked(trackId);
  if (x >= timelineLaneOriginX()) {
    emit scrubRequested(timelineSecondsForPosition(x, scrollSeconds, pixelsPerSecond, snapshot));
  }
  event->accept();
}

void TimelineSceneItem::mouseMoveEvent(QMouseEvent* event)
{
  if (event == nullptr || !m_scrubbingRuler) {
    return;
  }
  const SceneSnapshot emptySnapshot;
  const SceneSnapshot& snapshot = currentSnapshot(m_snapshot, emptySnapshot);
  emit scrubRequested(timelineSecondsForPosition(
    event->position().x(),
    itemFiniteNonNegative(m_viewportScrollSeconds),
    itemFinitePositive(m_viewportPixelsPerSecond, 100.0),
    snapshot));
  event->accept();
}

void TimelineSceneItem::mouseReleaseEvent(QMouseEvent* event)
{
  if (event == nullptr || !m_scrubbingRuler) {
    return;
  }
  const SceneSnapshot emptySnapshot;
  const SceneSnapshot& snapshot = currentSnapshot(m_snapshot, emptySnapshot);
  emit scrubRequested(timelineSecondsForPosition(
    event->position().x(),
    itemFiniteNonNegative(m_viewportScrollSeconds),
    itemFinitePositive(m_viewportPixelsPerSecond, 100.0),
    snapshot));
  m_scrubbingRuler = false;
  event->accept();
}

void TimelineSceneItem::wheelEvent(QWheelEvent* event)
{
  if (event == nullptr) {
    return;
  }

  const double zoomFactor = timelineZoomFactor(*event);
  if (!itemSameDouble(zoomFactor, 1.0)) {
    emit viewportZoomRequested(zoomFactor, timelineZoomAnchorX(event->position().x()));
    event->accept();
    return;
  }

  const double scrollDelta = timelineHorizontalScrollDelta(*event);
  if (hasMeaningfulDelta(scrollDelta)) {
    emit viewportScrollRequested(scrollDelta);
    event->accept();
    return;
  }

  const double verticalDelta = timelineVerticalScrollDelta(*event);
  if (hasMeaningfulDelta(verticalDelta)) {
    emit viewportVerticalScrollRequested(verticalDelta);
    event->accept();
    return;
  }

  event->ignore();
}
