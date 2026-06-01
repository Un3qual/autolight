import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

RowLayout {
    id: root
    property var appController
    property color controlTextColor: "#f4f4f5"
    property color secondaryText: "#d4d4d8"
    property var formatSeconds
    signal nudgeRequested(real delta)
    signal togglePlaybackRequested()
    signal stopRequested()
    signal volumeRequested(real value)
    signal seekRequested(real value)

    Layout.leftMargin: 12
    Layout.rightMargin: 12
    spacing: 8

    RowLayout {
        id: playbackControls
        spacing: 6

        Button {
            text: "-1s"
            palette.buttonText: root.controlTextColor
            enabled: root.appController.playback.sourcePath.length > 0
            onClicked: root.nudgeRequested(-1.0)
        }

        Button {
            text: root.appController.playback.isPlaying ? "Pause" : "Play"
            palette.buttonText: root.controlTextColor
            enabled: root.appController.selectedTrackCanPlay || (root.appController.selectedTrackId.length === 0 && root.appController.playback.sourcePath.length > 0) || root.appController.playback.isPlaying
            onClicked: root.togglePlaybackRequested()
        }

        Button {
            text: "Stop"
            palette.buttonText: root.controlTextColor
            enabled: root.appController.playback.sourcePath.length > 0
            onClicked: root.stopRequested()
        }

        Button {
            text: "+1s"
            palette.buttonText: root.controlTextColor
            enabled: root.appController.playback.sourcePath.length > 0
            onClicked: root.nudgeRequested(1.0)
        }

        Slider {
            id: playbackVolumeSlider
            from: 0
            to: 1
            value: root.appController.playback.volume
            Layout.preferredWidth: 88
            onMoved: root.volumeRequested(value)
        }

        Label {
            id: playheadTimeLabel
            text: root.formatSeconds(root.appController.playback.positionSeconds) + " / " + root.formatSeconds(root.appController.playback.durationSeconds)
            color: root.secondaryText
            font.pixelSize: 12
        }
    }

    Slider {
        id: playbackScrubber
        Layout.fillWidth: true
        from: 0
        to: Math.max(0.01, root.appController.playback.durationSeconds)
        value: root.appController.playback.positionSeconds
        enabled: root.appController.playback.sourcePath.length > 0
        live: true
        onMoved: root.seekRequested(value)
    }
}
