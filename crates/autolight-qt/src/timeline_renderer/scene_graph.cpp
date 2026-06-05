#include "scene_graph.h"

#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QVector>
#include <QtQuick/QSGFlatColorMaterial>
#include <QtQuick/QSGGeometry>
#include <QtQuick/QSGGeometryNode>

#include <cmath>
#include <utility>

namespace {

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

float finiteFloat(const QJsonValue& value, double fallback)
{
  const double number = value.toDouble(fallback);
  return std::isfinite(number) ? static_cast<float>(number) : static_cast<float>(fallback);
}

QVector<BandSpec> parseBands(const QString& geometryJson, QString* error)
{
  if (geometryJson.trimmed().isEmpty()) {
    return {};
  }

  const QJsonDocument document = QJsonDocument::fromJson(geometryJson.toUtf8());
  if (!document.isObject()) {
    *error = QStringLiteral("geometry json is not an object");
    return {};
  }

  QVector<BandSpec> bands;
  const QJsonArray jsonBands = document.object().value(QStringLiteral("bands")).toArray();
  bands.reserve(jsonBands.size());
  for (const QJsonValue& bandValue : jsonBands) {
    const QJsonObject bandObject = bandValue.toObject();
    const QColor color(bandObject.value(QStringLiteral("color")).toString(QStringLiteral("#60a5fa")));
    if (!color.isValid()) {
      continue;
    }
    BandSpec band;
    band.color = color;
    const QJsonArray rects = bandObject.value(QStringLiteral("rects")).toArray();
    band.rects.reserve(rects.size());
    for (const QJsonValue& rectValue : rects) {
      const QJsonObject rectObject = rectValue.toObject();
      RectSpec rect{
        finiteFloat(rectObject.value(QStringLiteral("x")), 0.0),
        finiteFloat(rectObject.value(QStringLiteral("y")), 0.0),
        finiteFloat(rectObject.value(QStringLiteral("width")), 0.0),
        finiteFloat(rectObject.value(QStringLiteral("height")), 0.0),
      };
      if (rect.width <= 0.0F || rect.height <= 0.0F) {
        continue;
      }
      band.rects.push_back(rect);
    }
    if (!band.rects.isEmpty()) {
      bands.push_back(std::move(band));
    }
  }
  return bands;
}

void addRect(QSGGeometry::Point2D* vertices, int offset, const RectSpec& rect)
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

int childCount(QSGNode* root)
{
  int count = 0;
  for (QSGNode* child = root->firstChild(); child != nullptr; child = child->nextSibling()) {
    ++count;
  }
  return count;
}

QSGGeometryNode* childAt(QSGNode* root, int targetIndex)
{
  int index = 0;
  for (QSGNode* child = root->firstChild(); child != nullptr; child = child->nextSibling()) {
    if (index == targetIndex) {
      return static_cast<QSGGeometryNode*>(child);
    }
    ++index;
  }
  return nullptr;
}

QSGGeometryNode* createEmptyBandNode()
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
  if (auto* existing = childAt(root, index)) {
    return existing;
  }
  auto* node = createEmptyBandNode();
  root->appendChildNode(node);
  return node;
}

void trimBandNodes(QSGNode* root, int targetCount)
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
    addRect(vertices, static_cast<int>(index) * 6, band.rects[index]);
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

QSGNode* updateRootNode(QSGNode* root, const QVector<BandSpec>& bands)
{
  for (int index = 0; index < bands.size(); ++index) {
    updateBandNode(ensureBandNode(root, index), bands[index]);
  }
  trimBandNodes(root, bands.size());
  return root;
}

} // namespace

TimelineGeometryItem::TimelineGeometryItem(QQuickItem* parent)
  : QQuickItem(parent)
{
  setFlag(QQuickItem::ItemHasContents, true);
  setAcceptedMouseButtons(Qt::NoButton);
}

QString TimelineGeometryItem::geometryJson() const
{
  return m_geometryJson;
}

void TimelineGeometryItem::setGeometryJson(const QString& geometryJson)
{
  if (m_geometryJson == geometryJson) {
    return;
  }
  m_geometryJson = geometryJson;
  update();
  emit geometryJsonChanged();
}

QString TimelineGeometryItem::emptyReason() const
{
  return m_emptyReason;
}

QSGNode* TimelineGeometryItem::updatePaintNode(QSGNode* oldNode, UpdatePaintNodeData*)
{
  QString error;
  const QVector<BandSpec> bands = parseBands(m_geometryJson, &error);
  m_emptyReason = error;
  if (bands.isEmpty()) {
    delete oldNode;
    return nullptr;
  }
  QSGNode* root = oldNode != nullptr ? oldNode : new QSGNode();
  return updateRootNode(root, bands);
}
