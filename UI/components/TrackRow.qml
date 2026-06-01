import QtQuick
import QtQuick.Controls

Row {
    id: root
    required property int index
    required property string trackId
    required property string name
    required property string trackType
    required property string resultState
    required property int markerCount
    required property int cacheRefCount
    required property string artifactKinds
    required property string error
    required property real jobProgress
    required property string activeJobId
    required property var markerSpans
    required property var waveformSamples
    required property real waveformDurationSeconds
    property var appController
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
    signal trackSelected(string trackId)
    signal seekRequested(real x)

    height: root.timelineRowHeight
    spacing: 0

    Rectangle {
        width: root.timelineLabelWidth
        height: parent.height
        color: root.index % 2 === 0 ? root.panelBackground : root.laneBackground
        border.color: root.appController.selectedTrackId === root.trackId ? root.focusAccent : root.borderSubtle

        Column {
            anchors.fill: parent
            anchors.margins: 10
            spacing: 4

            Text {
                text: root.name
                color: root.textPrimary
                font.pixelSize: 14
                elide: Text.ElideRight
                width: parent.width
            }

            Text {
                text: root.trackType + " - " + root.resultState + " - " + root.markerCount + " markers"
                color: root.resultState === "failed" || root.resultState === "stale" ? root.statusErrorColor : root.textMuted
                font.pixelSize: 12
                elide: Text.ElideRight
                width: parent.width
            }

            Text {
                text: root.cacheRefCount > 0 ? root.artifactKinds + " artifact" : ""
                color: root.artifactAccent
                font.pixelSize: 12
                elide: Text.ElideRight
                width: parent.width
                visible: root.cacheRefCount > 0
            }

            Text {
                text: root.error
                visible: root.error.length > 0
                color: "#fca5a5"
                font.pixelSize: 11
                elide: Text.ElideRight
                width: parent.width
            }

            ProgressBar {
                width: parent.width
                from: 0
                to: 1
                value: root.jobProgress
                visible: root.activeJobId.length > 0
            }
        }

        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton
            onClicked: root.trackSelected(root.trackId)
        }
    }

    TimelineLane {
        width: Math.max(0, parent.width - root.timelineLabelWidth)
        height: parent.height
        appController: root.appController
        rowIndex: root.index
        trackId: root.trackId
        markerSpans: root.markerSpans
        waveformSamples: root.waveformSamples
        waveformDurationSeconds: root.waveformDurationSeconds
        editable: root.trackType === "editable"
        timelineLeftPadding: root.timelineLeftPadding
        laneBackground: root.laneBackground
        laneBackgroundAlt: root.laneBackgroundAlt
        borderSubtle: root.borderSubtle
        focusAccent: root.focusAccent
        markerLabelText: root.markerLabelText
        onClicked: function(x) {
            root.trackSelected(root.trackId)
            root.seekRequested(x)
        }
    }
}
