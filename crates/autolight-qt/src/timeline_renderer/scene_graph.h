#pragma once

#include <QtCore/QString>
#include <QtGui/QColor>
#include <QtQuick/QQuickItem>
#include <QtQuick/QSGNode>
#include <QtQml/qqmlregistration.h>

class TimelineGeometryItem : public QQuickItem
{
  Q_OBJECT
  Q_PROPERTY(QString geometryJson READ geometryJson WRITE setGeometryJson NOTIFY geometryJsonChanged)
  Q_PROPERTY(QString emptyReason READ emptyReason NOTIFY geometryJsonChanged)

public:
  explicit TimelineGeometryItem(QQuickItem* parent = nullptr);

  QString geometryJson() const;
  void setGeometryJson(const QString& geometryJson);
  QString emptyReason() const;

signals:
  void geometryJsonChanged();

protected:
  QSGNode* updatePaintNode(QSGNode* oldNode, UpdatePaintNodeData* updateData) override;

private:
  QString m_geometryJson;
  QString m_emptyReason;
};

class TimelineWaveformItem : public TimelineGeometryItem
{
  Q_OBJECT
  QML_NAMED_ELEMENT(TimelineWaveformItem)

public:
  using TimelineGeometryItem::TimelineGeometryItem;
};

class TimelineAnalysisItem : public TimelineGeometryItem
{
  Q_OBJECT
  QML_NAMED_ELEMENT(TimelineAnalysisItem)

public:
  using TimelineGeometryItem::TimelineGeometryItem;
};
