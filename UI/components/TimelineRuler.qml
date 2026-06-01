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

    function tickStepSeconds() {
        var pixelsPerSecond = Number(timelineRuler.appController.timelinePixelsPerSecond)
        if (!isFinite(pixelsPerSecond) || pixelsPerSecond <= 0) {
            return 1
        }
        if (pixelsPerSecond >= 320) return 0.1
        if (pixelsPerSecond >= 160) return 0.25
        if (pixelsPerSecond >= 80) return 0.5
        if (pixelsPerSecond >= 40) return 1
        if (pixelsPerSecond >= 20) return 2
        if (pixelsPerSecond >= 10) return 5
        return 10
    }

    function formatTick(seconds) {
        var step = timelineRuler.tickStepSeconds()
        if (step < 1) {
            return seconds.toFixed(2).replace(/0+$/, "").replace(/\.$/, "") + "s"
        }
        return Math.round(seconds) + "s"
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
        id: timelineTickLane
        Layout.fillWidth: true
        Layout.fillHeight: true
        color: timelineRuler.panelBackground
        property real secondsPerTick: timelineRuler.tickStepSeconds()
        property real firstTickSecond: Math.floor(timelineRuler.appController.timelineScrollSeconds / secondsPerTick) * secondsPerTick

        Repeater {
            model: Math.ceil(timelineRuler.appController.timelineVisibleSeconds / timelineTickLane.secondsPerTick) + 2
            Text {
                property real tickSecond: timelineTickLane.firstTickSecond + index * timelineTickLane.secondsPerTick
                x: timelineRuler.timelineX(tickSecond)
                y: 9
                text: timelineRuler.formatTick(tickSecond)
                color: timelineRuler.textMuted
                font.pixelSize: 12
            }
        }
    }
}
