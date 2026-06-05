import QtQuick
import Autolight.Qt 1.0

Item {
    id: timelineRoot
    property var appController
    property real rowsWidth: width
    property real timelineLeftPadding: 24
    property real timelineLabelWidth: 280
    property int timelineRowHeight: 76
    property real timelineRulerHeight: 32
    property real trackScrollPixels: 0
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

    readonly property int trackCount: timelineRoot.appController && Array.isArray(timelineRoot.appController.trackRows)
        ? timelineRoot.appController.trackRows.length
        : 0

    function maxTrackScrollPixels() {
        var rowHeight = Math.max(1, timelineRoot.timelineRowHeight)
        var rowViewportHeight = Math.max(0, timelineRoot.height - timelineRoot.timelineRulerHeight)
        return Math.max(0, timelineRoot.trackCount * rowHeight - rowViewportHeight)
    }

    function setTrackScrollPixels(value) {
        var safeValue = Number(value)
        if (!isFinite(safeValue)) safeValue = 0
        timelineRoot.trackScrollPixels = Math.max(0, Math.min(timelineRoot.maxTrackScrollPixels(), safeValue))
    }

    function updateVisibleTrackRange() {
        if (!timelineRoot.appController) return
        if (typeof timelineRoot.appController.set_timeline_visible_track_range !== "function") return
        var rowHeight = Math.max(1, timelineRoot.timelineRowHeight)
        var firstRow = Math.max(0, Math.floor(timelineRoot.trackScrollPixels / rowHeight))
        var rowViewportHeight = Math.max(0, timelineRoot.height - timelineRoot.timelineRulerHeight)
        var rowCount = Math.max(0, Math.ceil(rowViewportHeight / rowHeight) + 2)
        timelineRoot.appController.set_timeline_visible_track_range(firstRow, rowCount)
    }

    function extendNativeViewportGesture() {
        if (!timelineRoot.appController) return
        timelineRoot.appController.begin_timeline_user_navigation()
        nativeViewportGestureQuietTimer.restart()
    }

    clip: true
    onWidthChanged: timelineRoot.layoutWidthChanged()
    onHeightChanged: timelineRoot.setTrackScrollPixels(timelineRoot.trackScrollPixels)
    onAppControllerChanged: timelineRoot.updateVisibleTrackRange()
    onTrackCountChanged: timelineRoot.setTrackScrollPixels(timelineRoot.trackScrollPixels)
    onTrackScrollPixelsChanged: timelineRoot.updateVisibleTrackRange()
    Component.onCompleted: timelineRoot.updateVisibleTrackRange()

    TimelineSceneItem {
        id: scene
        anchors.fill: parent
        sceneSnapshotJson: timelineRoot.appController ? timelineRoot.appController.timelineSceneSnapshotJson : ""
        viewportScrollSeconds: timelineRoot.appController ? timelineRoot.appController.timelineScrollSeconds : 0
        viewportPixelsPerSecond: timelineRoot.appController ? timelineRoot.appController.timelinePixelsPerSecond : 96
        viewportVisibleSeconds: timelineRoot.appController ? timelineRoot.appController.timelineVisibleSeconds : 8
        viewportTrackScrollPixels: timelineRoot.trackScrollPixels
        playbackPositionSeconds: timelineRoot.appController ? timelineRoot.appController.playback.positionSeconds : 0
        onTrackClicked: function(trackId) { timelineRoot.trackSelected(trackId) }
        onMarkerClicked: function(trackId, markerId, additive) {
            if (!timelineRoot.appController) return
            timelineRoot.appController.select_track(trackId)
            timelineRoot.appController.toggle_marker_selection(markerId, additive)
        }
        onTrackExpansionToggled: function(trackId, expanded) {
            if (timelineRoot.appController) timelineRoot.appController.set_track_expanded(trackId, expanded)
        }
        onScrubRequested: function(seconds) {
            var x = (seconds - scene.viewportScrollSeconds) * Math.max(1, scene.viewportPixelsPerSecond) + timelineRoot.timelineLeftPadding
            timelineRoot.seekRequested(x)
        }
        onViewportVerticalScrollRequested: function(pixelDelta) {
            timelineRoot.extendNativeViewportGesture()
            timelineRoot.setTrackScrollPixels(timelineRoot.trackScrollPixels + pixelDelta)
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
