import QtQuick

Rectangle {
    id: root
    property var appController
    property int rowIndex: 0
    property string trackId: ""
    property var markerSpans: []
    property var visibleWaveformSamples: []
    property var visibleEnergySamples: []
    property var visibleHarmonicColorSamples: []
    property real waveformDurationSeconds: 0
    property bool editable: false
    property real timelineLeftPadding: 24
    property color laneBackground: "#171a20"
    property color laneBackgroundAlt: "#14171d"
    property color borderSubtle: "#2f333d"
    property color focusAccent: "#facc15"
    property color markerLabelText: "#111318"
    signal clicked(real x)

    function timelineX(seconds) {
        return root.timelineLeftPadding + (seconds - root.appController.timelineScrollSeconds) * root.appController.timelinePixelsPerSecond
    }

    function snapTimelineTime(seconds, bypassSnap) {
        return root.appController.snap_timeline_time(seconds, bypassSnap)
    }

    color: root.rowIndex % 2 === 0 ? root.laneBackground : root.laneBackgroundAlt
    border.color: root.appController.selectedTrackId === root.trackId ? root.focusAccent : root.borderSubtle
    clip: true

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton
        onClicked: function(mouse) {
            root.clicked(mouse.x)
        }
    }

    WaveformStrip {
        anchors.fill: parent
        samples: root.visibleWaveformSamples
        durationSeconds: root.waveformDurationSeconds
        scrollSeconds: root.appController.timelineScrollSeconds
        pixelsPerSecond: root.appController.timelinePixelsPerSecond
        leftPadding: root.timelineLeftPadding
        visible: root.visibleWaveformSamples.length > 0
    }

    AnalysisStrip {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 18
        samples: root.visibleEnergySamples
        stripKind: "energy"
        scrollSeconds: root.appController.timelineScrollSeconds
        pixelsPerSecond: root.appController.timelinePixelsPerSecond
        leftPadding: root.timelineLeftPadding
    }

    AnalysisStrip {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 2
        samples: root.visibleHarmonicColorSamples
        stripKind: "harmonic-color"
        scrollSeconds: root.appController.timelineScrollSeconds
        pixelsPerSecond: root.appController.timelinePixelsPerSecond
        leftPadding: root.timelineLeftPadding
    }

    Repeater {
        model: markerSpans
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
        }
    }

    Rectangle {
        id: playhead
        width: 2
        height: parent.height
        x: root.timelineX(root.appController.playback.positionSeconds)
        color: root.focusAccent
        visible: root.appController.playback.sourcePath.length > 0
            && x >= root.timelineLeftPadding
            && x <= parent.width
        z: 10
    }
}
