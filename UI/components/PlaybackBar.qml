import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Basic as Basic
import QtQuick.Layouts

RowLayout {
    id: root
    property var appController
    property color controlTextColor: "#f4f4f5"
    property color secondaryText: "#d4d4d8"
    property color controlButtonBackground: "#242a35"
    property color controlButtonHover: "#2d3442"
    property color controlButtonPressed: "#353d4d"
    property color controlBorder: "#3a414f"
    property color controlTrack: "#2b313c"
    property color playAccent: "#facc15"
    property color scrubAccent: "#93c5fd"
    property var formatSeconds
    signal nudgeRequested(real delta)
    signal togglePlaybackRequested()
    signal stopRequested()
    signal volumeRequested(real value)
    signal seekRequested(real value)

    function validPlaybackDuration(durationSeconds) {
        var numericDuration = Number(durationSeconds)
        if (!isFinite(numericDuration) || numericDuration <= 0) {
            return 0.01
        }
        return Math.max(0.01, numericDuration)
    }

    function clampedPlaybackPosition(positionSeconds, durationSeconds) {
        var numericPosition = Number(positionSeconds)
        if (!isFinite(numericPosition)) {
            numericPosition = 0
        }
        return Math.max(0, Math.min(root.validPlaybackDuration(durationSeconds), numericPosition))
    }

    Layout.leftMargin: 12
    Layout.rightMargin: 12
    spacing: 8

    RowLayout {
        id: playbackControls
        spacing: 6

        Basic.Button {
            id: rewindButton
            text: "-1s"
            enabled: root.appController.playback.sourcePath.length > 0
            onClicked: root.nudgeRequested(-1.0)
            contentItem: Text {
                text: rewindButton.text
                color: rewindButton.enabled ? root.controlTextColor : "#737987"
                font.pixelSize: 12
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
            }
            background: Rectangle {
                radius: 4
                color: !rewindButton.enabled ? "#1d222b" : rewindButton.down ? root.controlButtonPressed : rewindButton.hovered ? root.controlButtonHover : root.controlButtonBackground
                border.color: root.controlBorder
            }
        }

        Basic.Button {
            id: playPauseButton
            text: root.appController.playback.isPlaying ? "Pause" : "Play"
            enabled: root.appController.selectedTrackCanPlay || (root.appController.selectedTrackId.length === 0 && root.appController.playback.sourcePath.length > 0) || root.appController.playback.isPlaying
            onClicked: root.togglePlaybackRequested()
            contentItem: Text {
                text: playPauseButton.text
                color: playPauseButton.enabled ? "#111318" : "#737987"
                font.pixelSize: 12
                font.bold: true
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
            }
            background: Rectangle {
                radius: 4
                color: !playPauseButton.enabled ? "#1d222b" : playPauseButton.down ? "#eab308" : root.playAccent
                border.color: playPauseButton.enabled ? "#fef08a" : root.controlBorder
            }
        }

        Basic.Button {
            id: stopButton
            text: "Stop"
            enabled: root.appController.playback.sourcePath.length > 0
            onClicked: root.stopRequested()
            contentItem: Text {
                text: stopButton.text
                color: stopButton.enabled ? root.controlTextColor : "#737987"
                font.pixelSize: 12
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
            }
            background: Rectangle {
                radius: 4
                color: !stopButton.enabled ? "#1d222b" : stopButton.down ? root.controlButtonPressed : stopButton.hovered ? root.controlButtonHover : root.controlButtonBackground
                border.color: root.controlBorder
            }
        }

        Basic.Button {
            id: forwardButton
            text: "+1s"
            enabled: root.appController.playback.sourcePath.length > 0
            onClicked: root.nudgeRequested(1.0)
            contentItem: Text {
                text: forwardButton.text
                color: forwardButton.enabled ? root.controlTextColor : "#737987"
                font.pixelSize: 12
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
            }
            background: Rectangle {
                radius: 4
                color: !forwardButton.enabled ? "#1d222b" : forwardButton.down ? root.controlButtonPressed : forwardButton.hovered ? root.controlButtonHover : root.controlButtonBackground
                border.color: root.controlBorder
            }
        }

        Basic.Slider {
            id: playbackVolumeSlider
            from: 0
            to: 1
            value: root.appController.playback.volume
            Layout.preferredWidth: 88
            onMoved: root.volumeRequested(value)
            background: Rectangle {
                x: playbackVolumeSlider.leftPadding
                y: playbackVolumeSlider.topPadding + playbackVolumeSlider.availableHeight / 2 - height / 2
                width: playbackVolumeSlider.availableWidth
                height: 4
                radius: 2
                color: root.controlTrack

                Rectangle {
                    width: playbackVolumeSlider.visualPosition * parent.width
                    height: parent.height
                    radius: parent.radius
                    color: root.secondaryText
                }
            }
            handle: Rectangle {
                x: playbackVolumeSlider.leftPadding + playbackVolumeSlider.visualPosition * (playbackVolumeSlider.availableWidth - width)
                y: playbackVolumeSlider.topPadding + playbackVolumeSlider.availableHeight / 2 - height / 2
                width: 12
                height: 12
                radius: 6
                color: root.secondaryText
                border.color: "#111318"
            }
        }

        Label {
            id: playheadTimeLabel
            text: root.formatSeconds(root.appController.playback.positionSeconds) + " / " + root.formatSeconds(root.appController.playback.durationSeconds)
            color: root.secondaryText
            font.pixelSize: 12
        }
    }

    Basic.Slider {
        id: playbackScrubber
        property bool scrubbing: pressed
        property real previewValue: root.clampedPlaybackPosition(root.appController.playback.positionSeconds, root.appController.playback.durationSeconds)
        property string pressedSourcePath: ""
        property real pressedDurationSeconds: root.validPlaybackDuration(root.appController.playback.durationSeconds)

        function resetPreview() {
            previewValue = root.clampedPlaybackPosition(root.appController.playback.positionSeconds, root.appController.playback.durationSeconds)
            pressedDurationSeconds = root.validPlaybackDuration(root.appController.playback.durationSeconds)
        }

        function clearPressCapture() {
            pressedSourcePath = ""
            resetPreview()
        }

        Layout.fillWidth: true
        from: 0
        to: root.validPlaybackDuration(root.appController.playback.durationSeconds)
        value: scrubbing ? previewValue : root.appController.playback.positionSeconds
        live: true
        enabled: root.appController.playback.sourcePath.length > 0
        onMoved: previewValue = root.clampedPlaybackPosition(value, root.appController.playback.durationSeconds)
        background: Rectangle {
            x: playbackScrubber.leftPadding
            y: playbackScrubber.topPadding + playbackScrubber.availableHeight / 2 - height / 2
            width: playbackScrubber.availableWidth
            height: 5
            radius: 2
            color: root.controlTrack

            Rectangle {
                width: playbackScrubber.visualPosition * parent.width
                height: parent.height
                radius: parent.radius
                color: root.scrubAccent
            }
        }
        handle: Rectangle {
            x: playbackScrubber.leftPadding + playbackScrubber.visualPosition * (playbackScrubber.availableWidth - width)
            y: playbackScrubber.topPadding + playbackScrubber.availableHeight / 2 - height / 2
            width: 14
            height: 14
            radius: 7
            color: playbackScrubber.enabled ? root.scrubAccent : "#535a67"
            border.color: "#111318"
        }
        onPressedChanged: {
            if (pressed) {
                pressedSourcePath = root.appController.playback.sourcePath
                pressedDurationSeconds = root.validPlaybackDuration(root.appController.playback.durationSeconds)
                previewValue = root.clampedPlaybackPosition(value, pressedDurationSeconds)
                return
            }

            var sourceStillLoaded = pressedSourcePath.length > 0
                && root.appController.playback.sourcePath === pressedSourcePath
            if (sourceStillLoaded) {
                var releaseDurationSeconds = Math.min(
                    pressedDurationSeconds,
                    root.validPlaybackDuration(root.appController.playback.durationSeconds)
                )
                root.seekRequested(root.clampedPlaybackPosition(previewValue, releaseDurationSeconds))
            }
            clearPressCapture()
        }

        Connections {
            target: root.appController.playback

            function onSourcePathChanged() {
                playbackScrubber.clearPressCapture()
            }

            function onDurationSecondsChanged() {
                playbackScrubber.resetPreview()
            }
        }
    }
}
