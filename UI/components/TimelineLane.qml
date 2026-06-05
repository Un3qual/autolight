import QtQuick

Rectangle {
    id: root
    property var appController
    property int rowIndex: 0
    property string trackId: ""
    property var markerSpans: []
    property var waveformRef: null
    property var analysisRefs: []
    property real waveformDurationSeconds: 0
    property bool editable: false
    readonly property bool rowSelected: root.appController.selectedTrackId === root.trackId
    property real timelineLeftPadding: 24
    property color laneBackground: "#171a20"
    property color laneBackgroundAlt: "#14171d"
    property color borderSubtle: "#2f333d"
    property color focusAccent: "#facc15"
    property color markerLabelText: "#111318"
    readonly property real timelineContentWidth: Math.max(0, width - root.timelineLeftPadding)
    readonly property real renderTileWidth: root.timelineContentWidth > 0 ? root.timelineContentWidth * 3 : 0
    readonly property real renderTileStepSeconds: root.timelineContentWidth > 0 && root.appController.timelinePixelsPerSecond > 0
        ? root.timelineContentWidth / root.appController.timelinePixelsPerSecond
        : 0
    readonly property real renderTileStartSeconds: root.quantizedTileStartSeconds(
        root.appController.timelineScrollSeconds,
        root.renderTileStepSeconds
    )
    readonly property real renderTileOffsetX: root.timelineLeftPadding
        + (root.renderTileStartSeconds - root.appController.timelineScrollSeconds)
            * root.appController.timelinePixelsPerSecond
    signal clicked(real x)
    signal scrubRequested(real x, real laneWidth)

    function timelineX(seconds) {
        return root.timelineLeftPadding + (seconds - root.appController.timelineScrollSeconds) * root.appController.timelinePixelsPerSecond
    }

    function snapTimelineTime(seconds, bypassSnap) {
        return root.appController.snap_timeline_time(seconds, bypassSnap)
    }

    function listOrEmpty(value) {
        return Array.isArray(value) ? value : []
    }

    function quantizedTileStartSeconds(scrollSeconds, stepSeconds) {
        var safeScroll = Number(scrollSeconds)
        var safeStep = Number(stepSeconds)
        if (!isFinite(safeScroll) || safeScroll <= 0) {
            return 0
        }
        if (!isFinite(safeStep) || safeStep <= 0) {
            return safeScroll
        }
        return Math.max(0, Math.floor(safeScroll / safeStep) * safeStep - safeStep)
    }

    color: root.rowIndex % 2 === 0 ? root.laneBackground : root.laneBackgroundAlt
    border.color: root.rowSelected ? root.focusAccent : root.borderSubtle
    border.width: root.rowSelected ? 2 : 1
    clip: true

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton
        onClicked: function(mouse) {
            root.clicked(mouse.x)
        }
    }

    TimelineNavigationSurface {
        anchors.fill: parent
        appController: root.appController
        laneWidth: width
        contentLeftPadding: root.timelineLeftPadding
        allowScrub: true
        onScrubRequested: function(x, laneWidth) {
            root.scrubRequested(x, laneWidth)
        }
    }

    TimelineWaveformItem {
        x: root.renderTileOffsetX
        width: root.renderTileWidth
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        z: 1
        // Reference-only Python timeline path. The Rust runtime uses TimelineSceneItem
        // in TimelineView.qml; do not optimize this path for new Rust timeline work.
        geometryJson: root.appController && root.appController.nativeController && root.waveformRef
            ? root.appController.nativeController.renderTimelineWaveform(
                root.trackId,
                root.waveformRef.cacheRef || "",
                root.renderTileStartSeconds,
                root.appController.timelinePixelsPerSecond,
                width,
                height
            )
            : ""
        visible: root.waveformRef !== null
    }

    Repeater {
        model: root.listOrEmpty(root.analysisRefs)
        TimelineAnalysisItem {
            x: root.renderTileOffsetX
            width: root.renderTileWidth
            anchors.bottom: parent.bottom
            anchors.bottomMargin: modelData.artifactKind === "energy" ? 18 : 2
            height: 16
            z: 2
            geometryJson: root.appController && root.appController.nativeController
                ? root.appController.nativeController.renderTimelineAnalysis(
                    root.trackId,
                    modelData.cacheRef || "",
                    root.renderTileStartSeconds,
                    root.appController.timelinePixelsPerSecond,
                    width,
                    height
                )
                : ""
        }
    }

    Repeater {
        model: root.listOrEmpty(markerSpans)
        MarkerBlock {
            marker: modelData
            trackId: root.trackId
            markerId: modelData.id
            timestamp: modelData.timestamp
            duration: modelData.duration
            markerSelected: modelData.selected
            markerColor: modelData.color
            markerLabel: modelData.label
            editable: root.editable
            pixelsPerSecond: root.appController.timelinePixelsPerSecond
            appController: root.appController
            timelineLeftPadding: root.timelineLeftPadding
            markerLabelText: root.markerLabelText
            baseX: root.timelineX(modelData.timestamp)
            width: Math.max(8, (modelData.duration > 0 ? modelData.duration : 0.08) * root.appController.timelinePixelsPerSecond)
            height: parent.height - 18
            y: 9
            onSelected: function(markerId, additive) {
                root.appController.select_track(root.trackId)
                root.appController.toggle_marker_selection(markerId, additive)
            }
            z: 20
        }
    }

    Item {
        id: playhead
        width: 7
        height: parent.height
        x: root.timelineX(root.appController.playback.positionSeconds) - width / 2
        visible: root.appController.timelineDurationSeconds > 0
            && x >= root.timelineLeftPadding
            && x <= parent.width
        z: 10

        Rectangle {
            anchors.fill: parent
            color: root.focusAccent
            opacity: 0.16
        }

        Rectangle {
            x: parent.width / 2 - width / 2
            width: 2
            height: parent.height
            color: root.focusAccent
        }
    }
}
