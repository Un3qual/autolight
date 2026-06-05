import QtQuick
import Autolight.Qt 1.0

Item {
    id: timelineRoot
    property var appController
    property real rowsWidth: width
    property real timelineLeftPadding: 24
    property real timelineLabelWidth: 280
    property int timelineRowHeight: 76
    property color panelBackground: "#1c1f26"
    property color laneBackground: "#171a20"
    property color laneBackgroundAlt: "#14171d"
    property color borderSubtle: "#2f333d"
    property color textPrimary: "#f4f4f5"
    property color textMuted: "#a1a1aa"
    property color focusAccent: "#facc15"
    property color statusErrorColor: "#f87171"
    property color artifactAccent: "#93c5fd"
    property color markerLabelText: "#111318"
    signal layoutWidthChanged()
    signal trackSelected(string trackId)
    signal seekRequested(real x)

    function extendNativeViewportGesture() {
        if (!timelineRoot.appController) return
        timelineRoot.appController.begin_timeline_user_navigation()
        nativeViewportGestureQuietTimer.restart()
    }

    clip: true
    onWidthChanged: timelineRoot.layoutWidthChanged()
    onHeightChanged: {
        if (timelineRoot.appController) {
            timelineRoot.appController.set_timeline_visible_track_range(0, Math.ceil(height / Math.max(1, timelineRoot.timelineRowHeight)) + 1)
        }
    }
    Component.onCompleted: {
        if (timelineRoot.appController) {
            timelineRoot.appController.set_timeline_visible_track_range(0, Math.ceil(height / Math.max(1, timelineRoot.timelineRowHeight)) + 1)
        }
    }

    TimelineSceneItem {
        id: scene
        anchors.fill: parent
        sceneSnapshotJson: timelineRoot.appController ? timelineRoot.appController.timelineSceneSnapshotJson : ""
        viewportScrollSeconds: timelineRoot.appController ? timelineRoot.appController.timelineScrollSeconds : 0
        viewportPixelsPerSecond: timelineRoot.appController ? timelineRoot.appController.timelinePixelsPerSecond : 96
        viewportVisibleSeconds: timelineRoot.appController ? timelineRoot.appController.timelineVisibleSeconds : 8
        playbackPositionSeconds: timelineRoot.appController ? timelineRoot.appController.playback.positionSeconds : 0
        onTrackClicked: function(trackId) { timelineRoot.trackSelected(trackId) }
        onTrackExpansionToggled: function(trackId, expanded) {
            if (timelineRoot.appController) timelineRoot.appController.set_track_expanded(trackId, expanded)
        }
        onScrubRequested: function(seconds) {
            var x = (seconds - scene.viewportScrollSeconds) * Math.max(1, scene.viewportPixelsPerSecond) + timelineRoot.timelineLabelWidth + timelineRoot.timelineLeftPadding
            timelineRoot.seekRequested(x)
        }
        onViewportScrollRequested: function(pixelDelta) {
            timelineRoot.extendNativeViewportGesture()
            if (timelineRoot.appController) {
                timelineRoot.appController.scroll_timeline_by_pixels(pixelDelta)
            }
        }
        onViewportZoomRequested: function(factor, anchorX) {
            timelineRoot.extendNativeViewportGesture()
            if (timelineRoot.appController) {
                timelineRoot.appController.zoom_timeline_by_factor(
                    factor,
                    anchorX,
                    Math.max(0, width - timelineRoot.timelineLabelWidth - timelineRoot.timelineLeftPadding)
                )
            }
        }
    }

    Timer {
        id: nativeViewportGestureQuietTimer
        interval: 220
        repeat: false
        onTriggered: {
            if (timelineRoot.appController) timelineRoot.appController.end_timeline_user_navigation()
        }
    }
}
