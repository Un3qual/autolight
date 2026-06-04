import QtQuick

ListView {
    id: timelineRows
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
    signal scrollPixelsRequested(real pixels)
    signal zoomRequested(real x, real wheelDelta)

    function updateVisibleTrackRange() {
        if (!timelineRows.appController) {
            return
        }
        var rowHeight = Math.max(1, timelineRows.timelineRowHeight)
        var firstRow = Math.max(0, Math.floor(timelineRows.contentY / rowHeight))
        var rowCount = Math.max(0, Math.ceil(timelineRows.height / rowHeight) + 1)
        timelineRows.appController.set_timeline_visible_track_range(firstRow, rowCount)
    }

    function wheelDeltaValue(value) {
        var number = Number(value)
        return isFinite(number) ? number : 0
    }

    readonly property var safeTrackRows: timelineRows.appController && Array.isArray(timelineRows.appController.trackRows)
        ? timelineRows.appController.trackRows
        : []

    model: timelineRows.safeTrackRows.length
    clip: true
    onWidthChanged: timelineRows.layoutWidthChanged()
    onHeightChanged: timelineRows.updateVisibleTrackRange()
    onContentYChanged: timelineRows.updateVisibleTrackRange()
    onCountChanged: timelineRows.updateVisibleTrackRange()
    Component.onCompleted: timelineRows.updateVisibleTrackRange()

    delegate: TrackRow {
        property var rowData: timelineRows.safeTrackRows[index] || ({})
        width: timelineRows.width
        trackId: rowData.trackId || ""
        name: rowData.name || ""
        trackType: rowData.trackType || ""
        resultState: rowData.resultState || ""
        markerCount: Number(rowData.markerCount || 0)
        cacheRefCount: Number(rowData.cacheRefCount || 0)
        artifactKinds: rowData.artifactKinds || ""
        error: rowData.error || ""
        jobProgress: Number(rowData.jobProgress || 0)
        activeJobId: rowData.activeJobId || ""
        markerSpans: rowData.markerSpans || []
        waveformLevels: rowData.waveformLevels || []
        visibleEnergySamples: rowData.visibleEnergySamples || []
        visibleHarmonicColorSamples: rowData.visibleHarmonicColorSamples || []
        waveformDurationSeconds: Number(rowData.waveformDurationSeconds || 0)
        depth: Number(rowData.depth || 0)
        hasChildren: Boolean(rowData.hasChildren)
        expanded: Boolean(rowData.expanded)
        visibleChildStateSummary: rowData.visibleChildStateSummary || ""
        treeError: rowData.treeError || ""
        appController: timelineRows.appController
        timelineLeftPadding: timelineRows.timelineLeftPadding
        timelineLabelWidth: timelineRows.timelineLabelWidth
        timelineRowHeight: timelineRows.timelineRowHeight
        panelBackground: timelineRows.panelBackground
        laneBackground: timelineRows.laneBackground
        laneBackgroundAlt: timelineRows.laneBackgroundAlt
        borderSubtle: timelineRows.borderSubtle
        textPrimary: timelineRows.textPrimary
        textMuted: timelineRows.textMuted
        focusAccent: timelineRows.focusAccent
        statusErrorColor: timelineRows.statusErrorColor
        artifactAccent: timelineRows.artifactAccent
        markerLabelText: timelineRows.markerLabelText
        onTrackSelected: function(trackId) { timelineRows.trackSelected(trackId) }
        onSeekRequested: function(x) { timelineRows.seekRequested(x) }
    }

    WheelHandler {
        target: null

        onWheel: function(event) {
            var pixelX = timelineRows.wheelDeltaValue(event.pixelDelta.x)
            var pixelY = timelineRows.wheelDeltaValue(event.pixelDelta.y)
            var angleX = timelineRows.wheelDeltaValue(event.angleDelta.x) / 8
            var angleY = timelineRows.wheelDeltaValue(event.angleDelta.y) / 8
            var zoomModifier = (event.modifiers & Qt.ControlModifier) !== 0
                || (event.modifiers & Qt.MetaModifier) !== 0
            if (zoomModifier) {
                var zoomDelta = pixelY !== 0 ? pixelY : angleY
                if (zoomDelta !== 0) {
                    timelineRows.zoomRequested(event.position.x, zoomDelta)
                    event.accepted = true
                    return
                }
                event.accepted = false
                return
            }

            var horizontalPixels = pixelX !== 0 ? -pixelX : -angleX
            if (horizontalPixels === 0 && (event.modifiers & Qt.ShiftModifier) !== 0) {
                horizontalPixels = pixelY !== 0 ? -pixelY : -angleY
            }
            if (horizontalPixels !== 0) {
                timelineRows.scrollPixelsRequested(horizontalPixels)
                event.accepted = true
                return
            }
            event.accepted = false
        }
    }
}
