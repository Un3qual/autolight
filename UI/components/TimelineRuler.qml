import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

RowLayout {
    id: timelineRuler
    property var appController
    property real timelineLeftPadding: 24
    property real timelineLabelWidth: 280
    property real timelineRulerHeight: 32
    property color panelBackground: "#1c1f26"
    property color borderSubtle: "#2f333d"
    property color textMuted: "#a1a1aa"

    function timelineX(seconds) {
        return timelineRuler.timelineLeftPadding + (seconds - timelineRuler.appController.timelineScrollSeconds) * timelineRuler.appController.timelinePixelsPerSecond
    }

    Layout.minimumHeight: timelineRuler.timelineRulerHeight
    Layout.preferredHeight: timelineRuler.timelineRulerHeight
    Layout.maximumHeight: timelineRuler.timelineRulerHeight
    spacing: 0

    Rectangle {
        Layout.preferredWidth: timelineRuler.timelineLabelWidth
        Layout.fillHeight: true
        color: timelineRuler.panelBackground
        border.color: timelineRuler.borderSubtle
    }

    Rectangle {
        Layout.fillWidth: true
        Layout.fillHeight: true
        color: timelineRuler.panelBackground

        Repeater {
            model: Math.ceil(timelineRuler.appController.timelineVisibleSeconds) + 1
            Text {
                property real tickSecond: Math.ceil(timelineRuler.appController.timelineScrollSeconds) + index
                x: timelineRuler.timelineX(tickSecond)
                y: 9
                text: tickSecond + "s"
                color: timelineRuler.textMuted
                font.pixelSize: 12
            }
        }
    }
}
