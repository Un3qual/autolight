#include "scene_graph_nodes.h"

#include <QtCore/QRectF>
#include <QtCore/QSize>
#include <QtGui/QFont>
#include <QtGui/QFontMetrics>
#include <QtGui/QImage>
#include <QtGui/QPainter>
#include <QtQuick/QQuickWindow>
#include <QtQuick/QSGFlatColorMaterial>
#include <QtQuick/QSGGeometry>
#include <QtQuick/QSGGeometryNode>
#include <QtQuick/QSGNode>
#include <QtQuick/QSGSimpleTextureNode>

#include <algorithm>
#include <cmath>

namespace autolight::qt::timeline_scene {

namespace {

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

bool updateTextNode(TextTextureNode* node, const TextSpec& spec, QQuickWindow* window)
{
  if (node == nullptr || window == nullptr) {
    return false;
  }

  node->setRect(spec.rect);
  const double devicePixelRatio = std::max(1.0, window->effectiveDevicePixelRatio());
  const QSize imageSize(
    std::max(1, static_cast<int>(std::ceil(spec.rect.width() * devicePixelRatio))),
    std::max(1, static_cast<int>(std::ceil(spec.rect.height() * devicePixelRatio))));
  const QString key = QStringLiteral("%1|%2|%3").arg(spec.key).arg(imageSize.width()).arg(imageSize.height());
  if (node->key == key) {
    return false;
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
  return true;
}

void updateTextNodes(
  QSGNode* root,
  const QVector<TextSpec>& texts,
  QQuickWindow* window,
  SceneGraphUpdateStats& stats)
{
  if (window == nullptr) {
    trimChildNodes(root, 0);
    return;
  }
  for (int index = 0; index < texts.size(); ++index) {
    if (updateTextNode(ensureTextNode(root, index), texts[index], window)) {
      ++stats.textTexturesCreated;
    }
  }
  trimChildNodes(root, texts.size());
}

} // namespace

QSGNode* updateTimelineSceneGraph(
  QSGNode* root,
  const SceneFrameSpec& frame,
  QQuickWindow* window,
  SceneGraphUpdateStats* stats)
{
  QSGNode* geometryRoot = ensureContainerNode(root, 0);
  QSGNode* textRoot = ensureContainerNode(root, 1);

  for (int index = 0; index < frame.bands.size(); ++index) {
    updateBandNode(ensureBandNode(geometryRoot, index), frame.bands[index]);
  }
  trimChildNodes(geometryRoot, frame.bands.size());

  SceneGraphUpdateStats localStats;
  SceneGraphUpdateStats& effectiveStats = stats != nullptr ? *stats : localStats;
  updateTextNodes(textRoot, frame.texts, window, effectiveStats);
  trimChildNodes(root, 2);
  return root;
}

} // namespace autolight::qt::timeline_scene
