import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Basic as Basic

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
    required property var waveformRef
    required property var analysisRefs
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

    function stateColor(state) {
        if (state === "failed") return root.statusErrorColor
        if (state === "stale") return "#fbbf24"
        if (state === "complete") return "#86efac"
        if (state === "running" || state === "pending") return root.artifactAccent
        return root.textMuted
    }

    function trackTypeLabel(trackType) {
        if (trackType === "source") return "AUDIO"
        if (trackType === "editable") return "CUES"
        if (trackType === "generated") return "ANALYSIS"
        return trackType.toUpperCase()
    }

    function markerSummary() {
        return root.markerCount === 1 ? "1 marker" : root.markerCount + " markers"
    }

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
                id: trackTitleRow
                width: parent.width
                height: 22
                spacing: 6

                Rectangle {
                    id: disclosureButton
                    width: root.hasChildren ? 24 : 0
                    height: 22
                    visible: root.hasChildren
                    radius: 4
                    color: disclosureMouseArea.containsMouse ? "#303846" : "#252b36"
                    border.color: root.borderSubtle

                    Text {
                        anchors.centerIn: parent
                        text: root.expanded ? "▾" : "▸"
                        color: root.textMuted
                        font.pixelSize: 13
                    }

                    MouseArea {
                        id: disclosureMouseArea
                        anchors.fill: parent
                        hoverEnabled: true
                        acceptedButtons: Qt.LeftButton
                        onClicked: root.appController.set_track_expanded(root.trackId, !root.expanded)
                    }
                }

                Text {
                    text: root.name
                    color: root.textPrimary
                    font.pixelSize: 14
                    font.bold: root.rowSelected
                    elide: Text.ElideRight
                    verticalAlignment: Text.AlignVCenter
                    height: parent.height
                    width: parent.width - disclosureButton.width - 8
                }
            }

            Row {
                id: trackBadgeRow
                width: parent.width
                height: 18
                spacing: 6

                Rectangle {
                    id: trackTypeBadge
                    width: Math.max(52, trackTypeText.implicitWidth + 14)
                    height: 18
                    radius: 4
                    color: root.rowSelected ? "#3a3440" : "#252b36"
                    border.color: root.rowSelected ? root.focusAccent : root.borderSubtle

                    Text {
                        id: trackTypeText
                        anchors.centerIn: parent
                        text: root.trackTypeLabel(root.trackType)
                        color: root.rowSelected ? root.focusAccent : root.textMuted
                        font.pixelSize: 10
                        font.bold: true
                    }
                }

                Rectangle {
                    id: stateBadge
                    width: Math.max(64, stateText.implicitWidth + 14)
                    height: 18
                    radius: 4
                    color: "#1f2530"
                    border.color: root.stateColor(root.resultState)

                    Text {
                        id: stateText
                        anchors.centerIn: parent
                        text: root.resultState.toUpperCase()
                        color: root.stateColor(root.resultState)
                        font.pixelSize: 10
                        font.bold: true
                    }
                }

                Text {
                    text: root.markerSummary()
                    color: root.textMuted
                    font.pixelSize: 12
                    elide: Text.ElideRight
                    verticalAlignment: Text.AlignVCenter
                    height: parent.height
                    width: Math.max(0, parent.width - trackTypeBadge.width - stateBadge.width - 18)
                }
            }

            Text {
                text: root.treeError.length > 0 && root.error.length > 0
                    ? root.treeError + " - " + root.error
                    : root.treeError.length > 0 ? root.treeError : root.error.length > 0 ? root.error
                        : root.cacheRefCount > 0 ? root.artifactKinds + " artifact"
                        : root.visibleChildStateSummary.length > 0 ? "children " + root.visibleChildStateSummary
                        : ""
                visible: text.length > 0
                color: root.error.length > 0 || root.treeError.length > 0 ? "#fca5a5" : root.cacheRefCount > 0 ? root.artifactAccent : root.textMuted
                font.pixelSize: 11
                elide: Text.ElideRight
                width: parent.width
            }

            Basic.ProgressBar {
                width: parent.width
                height: 5
                from: 0
                to: 1
                value: root.jobProgress
                visible: root.activeJobId.length > 0
                background: Rectangle {
                    radius: 2
                    color: "#252b36"
                }
                contentItem: Item {
                    Rectangle {
                        width: parent.width * root.jobProgress
                        height: parent.height
                        radius: 2
                        color: root.artifactAccent
                    }
                }
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
        waveformRef: root.waveformRef
        analysisRefs: root.analysisRefs
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
