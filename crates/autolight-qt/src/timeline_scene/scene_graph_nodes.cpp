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

QSGNode* appendContainerNode(QSGNode* root)
{
  auto* node = new QSGNode();
  root->appendChildNode(node);
  return node;
}

QSGNode* ensureGeometryRoot(QSGNode* root)
{
  if (QSGNode* geometryRoot = root->firstChild()) {
    return geometryRoot;
  }
  return appendContainerNode(root);
}

QSGNode* ensureTextRoot(QSGNode* root)
{
  QSGNode* geometryRoot = ensureGeometryRoot(root);
  if (QSGNode* textRoot = geometryRoot->nextSibling()) {
    return textRoot;
  }
  return appendContainerNode(root);
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

QSGGeometryNode* nextBandNode(QSGNode* root, QSGNode*& nextChild)
{
  if (nextChild != nullptr) {
    auto* node = static_cast<QSGGeometryNode*>(nextChild);
    nextChild = nextChild->nextSibling();
    return node;
  }
  auto* node = createBandNode();
  root->appendChildNode(node);
  return node;
}

void trimChildNodesFrom(QSGNode* root, QSGNode* firstObsolete)
{
  while (firstObsolete != nullptr) {
    QSGNode* child = firstObsolete;
    firstObsolete = firstObsolete->nextSibling();
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

TextTextureNode* nextTextNode(QSGNode* root, QSGNode*& nextChild)
{
  if (nextChild != nullptr) {
    auto* node = static_cast<TextTextureNode*>(nextChild);
    nextChild = nextChild->nextSibling();
    return node;
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

  auto* texture = window->createTextureFromImage(image);
  if (texture == nullptr) {
    return false;
  }
  node->setTexture(texture);
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
    trimChildNodesFrom(root, root->firstChild());
    return;
  }
  QSGNode* nextTextChild = root->firstChild();
  for (int index = 0; index < texts.size(); ++index) {
    if (updateTextNode(nextTextNode(root, nextTextChild), texts[index], window)) {
      ++stats.textTexturesCreated;
    }
  }
  trimChildNodesFrom(root, nextTextChild);
}

} // namespace

QSGNode* updateTimelineSceneGraph(
  QSGNode* root,
  const SceneFrameSpec& frame,
  QQuickWindow* window,
  SceneGraphUpdateStats* stats)
{
  QSGNode* geometryRoot = ensureGeometryRoot(root);
  QSGNode* textRoot = ensureTextRoot(root);

  QSGNode* nextBandChild = geometryRoot->firstChild();
  for (int index = 0; index < frame.bands.size(); ++index) {
    updateBandNode(nextBandNode(geometryRoot, nextBandChild), frame.bands[index]);
  }
  trimChildNodesFrom(geometryRoot, nextBandChild);

  SceneGraphUpdateStats localStats;
  SceneGraphUpdateStats& effectiveStats = stats != nullptr ? *stats : localStats;
  updateTextNodes(textRoot, frame.texts, window, effectiveStats);
  trimChildNodesFrom(root, textRoot->nextSibling());
  return root;
}

} // namespace autolight::qt::timeline_scene
