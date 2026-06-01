import QtQuick

Item {
    id: root
    property var appController
    property var waveformSamples: []
    property real waveformDurationSeconds: 0
    property real timelineLeftPadding: 24
    property color borderSubtle: "#2f333d"

    function timelineX(seconds) {
        return root.timelineLeftPadding + (seconds - root.appController.timelineScrollSeconds) * root.appController.timelinePixelsPerSecond
    }

    Rectangle {
        id: waveformCenterLine
        x: root.timelineLeftPadding
        y: Math.round(parent.height / 2)
        width: Math.max(0, parent.width - root.timelineLeftPadding)
        height: 1
        color: root.borderSubtle
        visible: root.waveformSamples.length > 0
    }

    Repeater {
        model: waveformSamples
        Item {
            width: 3
            height: parent.height
            x: root.timelineX(index / Math.max(1, waveformSamples.length - 1) * waveformDurationSeconds)
            visible: x >= root.timelineLeftPadding - width && x <= parent.width

            Rectangle {
                width: 2
                height: Math.max(2, modelData.peak * (parent.height - 18))
                y: (parent.height - height) / 2
                color: "#60a5fa"
                opacity: 0.75
            }

            Rectangle {
                width: 2
                height: Math.max(2, modelData.rms * (parent.height - 18))
                y: (parent.height - height) / 2
                color: "#bfdbfe"
                opacity: 0.95
            }
        }
    }
}
