#pragma once

#include <QtCore/QString>
#include <QtGui/QMouseEvent>
#include <QtGui/QWheelEvent>
#include <QtQuick/QQuickItem>
#include <QtQuick/QSGNode>
#include <QtQml/qqmlregistration.h>

#include <memory>

struct TimelineSceneSnapshotData;

class TimelineSceneItem : public QQuickItem
{
  Q_OBJECT
  QML_NAMED_ELEMENT(TimelineSceneItem)
  Q_PROPERTY(QString sceneSnapshotJson READ sceneSnapshotJson WRITE setSceneSnapshotJson NOTIFY sceneSnapshotJsonChanged)
  Q_PROPERTY(double viewportScrollSeconds READ viewportScrollSeconds WRITE setViewportScrollSeconds NOTIFY viewportScrollSecondsChanged)
  Q_PROPERTY(double viewportPixelsPerSecond READ viewportPixelsPerSecond WRITE setViewportPixelsPerSecond NOTIFY viewportPixelsPerSecondChanged)
  Q_PROPERTY(double viewportVisibleSeconds READ viewportVisibleSeconds WRITE setViewportVisibleSeconds NOTIFY viewportVisibleSecondsChanged)
  Q_PROPERTY(double viewportTrackScrollPixels READ viewportTrackScrollPixels WRITE setViewportTrackScrollPixels NOTIFY viewportTrackScrollPixelsChanged)
  Q_PROPERTY(double playbackPositionSeconds READ playbackPositionSeconds WRITE setPlaybackPositionSeconds NOTIFY playbackPositionSecondsChanged)
  Q_PROPERTY(int selectedTrackIndex READ selectedTrackIndex WRITE setSelectedTrackIndex NOTIFY selectedTrackIndexChanged)
  Q_PROPERTY(qulonglong sceneSnapshotParseCount READ sceneSnapshotParseCount NOTIFY scenePerfCountersChanged)
  Q_PROPERTY(qulonglong worstSceneSnapshotParseMicros READ worstSceneSnapshotParseMicros NOTIFY scenePerfCountersChanged)
  Q_PROPERTY(qulonglong worstSceneGraphUpdateMicros READ worstSceneGraphUpdateMicros NOTIFY scenePerfCountersChanged)
  Q_PROPERTY(qulonglong textTextureCreateCount READ textTextureCreateCount NOTIFY scenePerfCountersChanged)

public:
  explicit TimelineSceneItem(QQuickItem* parent = nullptr);
  ~TimelineSceneItem() override;

  QString sceneSnapshotJson() const;
  void setSceneSnapshotJson(const QString& sceneSnapshotJson);

  double viewportScrollSeconds() const;
  void setViewportScrollSeconds(double viewportScrollSeconds);

  double viewportPixelsPerSecond() const;
  void setViewportPixelsPerSecond(double viewportPixelsPerSecond);

  double viewportVisibleSeconds() const;
  void setViewportVisibleSeconds(double viewportVisibleSeconds);

  double viewportTrackScrollPixels() const;
  void setViewportTrackScrollPixels(double viewportTrackScrollPixels);

  double playbackPositionSeconds() const;
  void setPlaybackPositionSeconds(double playbackPositionSeconds);

  int selectedTrackIndex() const;
  void setSelectedTrackIndex(int selectedTrackIndex);

  qulonglong sceneSnapshotParseCount() const;
  qulonglong worstSceneSnapshotParseMicros() const;
  qulonglong worstSceneGraphUpdateMicros() const;
  qulonglong textTextureCreateCount() const;

signals:
  void sceneSnapshotJsonChanged();
  void viewportScrollSecondsChanged();
  void viewportPixelsPerSecondChanged();
  void viewportVisibleSecondsChanged();
  void viewportTrackScrollPixelsChanged();
  void playbackPositionSecondsChanged();
  void selectedTrackIndexChanged();
  void scenePerfCountersChanged();
  void trackClicked(const QString& trackId);
  void markerClicked(const QString& trackId, const QString& markerId, bool additive);
  void trackExpansionToggled(const QString& trackId, bool expanded);
  void scrubRequested(double seconds);
  void viewportScrollRequested(double pixelDelta);
  void viewportVerticalScrollRequested(double pixelDelta);
  void viewportZoomRequested(double factor, double anchorX);

protected:
  QSGNode* updatePaintNode(QSGNode* oldNode, UpdatePaintNodeData* updateData) override;
  void mousePressEvent(QMouseEvent* event) override;
  void mouseMoveEvent(QMouseEvent* event) override;
  void mouseReleaseEvent(QMouseEvent* event) override;
  void wheelEvent(QWheelEvent* event) override;

private:
  QString m_sceneSnapshotJson;
  double m_viewportScrollSeconds = 0.0;
  double m_viewportPixelsPerSecond = 100.0;
  double m_viewportVisibleSeconds = 1.0;
  double m_viewportTrackScrollPixels = 0.0;
  double m_playbackPositionSeconds = 0.0;
  int m_selectedTrackIndex = -1;
  bool m_scrubbingRuler = false;
  qulonglong m_sceneSnapshotParseCount = 0;
  qulonglong m_worstSceneSnapshotParseMicros = 0;
  qulonglong m_worstSceneGraphUpdateMicros = 0;
  qulonglong m_textTextureCreateCount = 0;
  std::unique_ptr<TimelineSceneSnapshotData> m_snapshot;
};
