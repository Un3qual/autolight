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
    required property var waveformLevels
    required property var visibleEnergySamples
    required property var visibleHarmonicColorSamples
    required property real waveformDurationSeconds
    required property int depth
    required property bool hasChildren
    required property bool expanded
    required property string visibleChildStateSummary
    required property string treeError
    property var appController
    readonly property bool rowSelected: root.appController.selectedTrackId === root.trackId
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
        color: root.rowSelected ? "#242934" : root.index % 2 === 0 ? root.panelBackground : root.laneBackground
        border.color: root.rowSelected ? root.focusAccent : root.borderSubtle
        border.width: root.rowSelected ? 2 : 1

        Rectangle {
            id: selectedTrackStripe
            width: 4
            anchors.left: parent.left
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            color: root.focusAccent
            visible: root.rowSelected
            z: 2
        }

        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton
            onClicked: root.trackSelected(root.trackId)
        }

        Column {
            anchors.fill: parent
            property int leftPadding: 10 + root.depth * 18
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            anchors.right: parent.right
            anchors.left: parent.left
            anchors.leftMargin: leftPadding
            anchors.rightMargin: 10
            anchors.topMargin: 10
            anchors.bottomMargin: 10
            spacing: 4

            Row {
                width: parent.width
                spacing: 6

                Button {
                    width: 24
                    height: 22
                    visible: root.hasChildren
                    text: root.expanded ? "▾" : "▸"
                    onClicked: root.appController.set_track_expanded(root.trackId, !root.expanded)
                }

                Text {
                    text: root.name
                    color: root.textPrimary
                    font.pixelSize: 14
                    elide: Text.ElideRight
                    width: parent.width - (root.hasChildren ? 30 : 0)
                }
            }

            Text {
                text: root.visibleChildStateSummary.length > 0
                    ? root.trackType + " - " + root.resultState + " - " + root.markerCount + " markers - children " + root.visibleChildStateSummary
                    : root.trackType + " - " + root.resultState + " - " + root.markerCount + " markers"
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
                text: root.treeError.length > 0 && root.error.length > 0
                    ? root.treeError + " - " + root.error
                    : root.treeError.length > 0 ? root.treeError : root.error
                visible: root.error.length > 0 || root.treeError.length > 0
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
    }

    TimelineLane {
        width: Math.max(0, parent.width - root.timelineLabelWidth)
        height: parent.height
        appController: root.appController
        rowIndex: root.index
        trackId: root.trackId
        markerSpans: root.markerSpans
        waveformLevels: root.waveformLevels
        visibleEnergySamples: root.visibleEnergySamples
        visibleHarmonicColorSamples: root.visibleHarmonicColorSamples
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
