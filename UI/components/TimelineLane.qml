import QtQuick

Rectangle {
    id: root
    property var appController
    property int rowIndex: 0
    property string trackId: ""
    property var markerSpans: []
    property var waveformSamples: []
    property real waveformDurationSeconds: 0
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

    color: root.rowIndex % 2 === 0 ? root.laneBackground : root.laneBackgroundAlt
    border.color: root.appController.selectedTrackId === root.trackId ? root.focusAccent : root.borderSubtle
    clip: true

    WaveformStrip {
        anchors.fill: parent
        appController: root.appController
        waveformSamples: root.waveformSamples
        waveformDurationSeconds: root.waveformDurationSeconds
        timelineLeftPadding: root.timelineLeftPadding
        borderSubtle: root.borderSubtle
    }

    Repeater {
        model: markerSpans
        MarkerBlock {
            marker: modelData
            appController: root.appController
            timelineLeftPadding: root.timelineLeftPadding
            markerLabelText: root.markerLabelText
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

    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton
        onClicked: function(mouse) {
            root.clicked(mouse.x)
        }
    }
}
