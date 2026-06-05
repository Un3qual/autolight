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
  Q_PROPERTY(double playbackPositionSeconds READ playbackPositionSeconds WRITE setPlaybackPositionSeconds NOTIFY playbackPositionSecondsChanged)
  Q_PROPERTY(int selectedTrackIndex READ selectedTrackIndex WRITE setSelectedTrackIndex NOTIFY selectedTrackIndexChanged)

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

  double playbackPositionSeconds() const;
  void setPlaybackPositionSeconds(double playbackPositionSeconds);

  int selectedTrackIndex() const;
  void setSelectedTrackIndex(int selectedTrackIndex);

signals:
  void sceneSnapshotJsonChanged();
  void viewportScrollSecondsChanged();
  void viewportPixelsPerSecondChanged();
  void viewportVisibleSecondsChanged();
  void playbackPositionSecondsChanged();
  void selectedTrackIndexChanged();
  void trackClicked(const QString& trackId);
  void trackExpansionToggled(const QString& trackId, bool expanded);
  void scrubRequested(double seconds);
  void viewportScrollRequested(double pixelDelta);
  void viewportZoomRequested(double factor, double anchorX);

protected:
  QSGNode* updatePaintNode(QSGNode* oldNode, UpdatePaintNodeData* updateData) override;
  void mousePressEvent(QMouseEvent* event) override;
  void wheelEvent(QWheelEvent* event) override;

private:
  QString m_sceneSnapshotJson;
  double m_viewportScrollSeconds = 0.0;
  double m_viewportPixelsPerSecond = 100.0;
  double m_viewportVisibleSeconds = 1.0;
  double m_playbackPositionSeconds = 0.0;
  int m_selectedTrackIndex = -1;
  std::unique_ptr<TimelineSceneSnapshotData> m_snapshot;
};
