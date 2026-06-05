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
    property color focusAccent: "#facc15"
    readonly property bool hasController: timelineRuler.appController !== undefined
        && timelineRuler.appController !== null
    signal scrubRequested(real x, real laneWidth)

    function timelineX(seconds) {
        if (!timelineRuler.hasController) {
            return timelineRuler.timelineLeftPadding
        }
        return timelineRuler.timelineLeftPadding + (seconds - timelineRuler.appController.timelineScrollSeconds) * timelineRuler.appController.timelinePixelsPerSecond
    }

    function tickStepSeconds() {
        if (!timelineRuler.hasController) {
            return 1
        }
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

        Text {
            anchors.left: parent.left
            anchors.leftMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            text: "TIME"
            color: timelineRuler.textMuted
            font.pixelSize: 10
            font.bold: true
        }
    }

    Rectangle {
        id: timelineTickLane
        Layout.fillWidth: true
        Layout.fillHeight: true
        color: timelineRuler.panelBackground
        clip: true
        property real secondsPerTick: timelineRuler.tickStepSeconds()
        property real firstTickSecond: timelineRuler.hasController
            ? Math.floor(timelineRuler.appController.timelineScrollSeconds / secondsPerTick) * secondsPerTick
            : 0

        Repeater {
            model: timelineRuler.hasController
                ? Math.ceil(timelineRuler.appController.timelineVisibleSeconds / timelineTickLane.secondsPerTick) + 2
                : 0
            Item {
                property real tickSecond: timelineTickLane.firstTickSecond + index * timelineTickLane.secondsPerTick
                property bool majorTick: timelineTickLane.secondsPerTick >= 1
                    || Math.abs(tickSecond - Math.round(tickSecond)) < 0.0001
                property bool showLabel: timelineTickLane.secondsPerTick >= 1 || majorTick
                x: timelineRuler.timelineX(tickSecond)
                y: 0
                width: 1
                height: timelineTickLane.height

                Rectangle {
                    id: tickLine
                    width: 1
                    height: parent.majorTick ? parent.height : 9
                    y: parent.majorTick ? 0 : parent.height - height
                    color: parent.majorTick ? timelineRuler.borderSubtle : "#272d37"
                }

                Text {
                    x: 5
                    y: 8
                    visible: parent.showLabel
                    text: timelineRuler.formatTick(parent.tickSecond)
                    color: timelineRuler.textMuted
                    font.pixelSize: 11
                }
            }
        }

        TimelineNavigationSurface {
            anchors.fill: parent
            appController: timelineRuler.appController
            laneWidth: width
            contentLeftPadding: timelineRuler.timelineLeftPadding
            allowScrub: true
            onScrubRequested: function(x, laneWidth) {
                timelineRuler.scrubRequested(x, laneWidth)
            }
        }

        Item {
            id: playheadHandle
            width: 16
            height: parent.height
            z: 20
            visible: timelineRuler.hasController
                && timelineRuler.appController.timelineDurationSeconds > 0
            x: !timelineRuler.hasController
                ? timelineRuler.timelineLeftPadding
                : timelineRuler.appController.timelinePlayheadOffscreenDirection < 0
                ? timelineRuler.timelineLeftPadding
                : timelineRuler.appController.timelinePlayheadOffscreenDirection > 0
                    ? parent.width - width
                    : timelineRuler.timelineX(timelineRuler.appController.playback.positionSeconds) - width / 2

            Rectangle {
                id: playheadStem
                x: parent.width / 2 - width / 2
                width: 2
                height: parent.height
                color: timelineRuler.focusAccent
                opacity: 0.9
            }

            Rectangle {
                id: playheadCap
                x: 1
                y: 2
                width: 14
                height: 12
                radius: 3
                color: timelineRuler.focusAccent
                border.color: "#fef08a"
                border.width: 1
            }

            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.LeftButton
                onPressed: timelineRuler.appController.begin_timeline_user_navigation()
                onPositionChanged: function(mouse) {
                    if (pressed) {
                        timelineRuler.scrubRequested(
                            Math.max(0, playheadHandle.x + mouse.x - timelineRuler.timelineLeftPadding),
                            Math.max(0, timelineTickLane.width - timelineRuler.timelineLeftPadding)
                        )
                    }
                }
                onReleased: function(mouse) {
                    timelineRuler.scrubRequested(
                        Math.max(0, playheadHandle.x + mouse.x - timelineRuler.timelineLeftPadding),
                        Math.max(0, timelineTickLane.width - timelineRuler.timelineLeftPadding)
                    )
                    timelineRuler.appController.end_timeline_user_navigation()
                }
                onCanceled: timelineRuler.appController.end_timeline_user_navigation()
            }
        }
    }
}
