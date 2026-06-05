#pragma once

#include "scene_frame_builder.h"

#include <QtCore/QtGlobal>

class QQuickWindow;
class QSGNode;

namespace autolight::qt::timeline_scene {

struct SceneGraphUpdateStats
{
  qulonglong textTexturesCreated = 0;
};

QSGNode* updateTimelineSceneGraph(
  QSGNode* root,
  const SceneFrameSpec& frame,
  QQuickWindow* window,
  SceneGraphUpdateStats* stats);

} // namespace autolight::qt::timeline_scene
