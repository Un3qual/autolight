import QtQuick

ListView {
    id: timelineRows
    property var appController
    property real rowsWidth: width
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
    signal layoutWidthChanged()
    signal trackSelected(string trackId)
    signal seekRequested(real x)

    model: timelineRows.appController.trackModel
    clip: true
    onWidthChanged: timelineRows.layoutWidthChanged()

    delegate: TrackRow {
        width: timelineRows.width
        appController: timelineRows.appController
        timelineLeftPadding: timelineRows.timelineLeftPadding
        timelineLabelWidth: timelineRows.timelineLabelWidth
        timelineRowHeight: timelineRows.timelineRowHeight
        panelBackground: timelineRows.panelBackground
        laneBackground: timelineRows.laneBackground
        laneBackgroundAlt: timelineRows.laneBackgroundAlt
        borderSubtle: timelineRows.borderSubtle
        textPrimary: timelineRows.textPrimary
        textMuted: timelineRows.textMuted
        focusAccent: timelineRows.focusAccent
        statusErrorColor: timelineRows.statusErrorColor
        artifactAccent: timelineRows.artifactAccent
        markerLabelText: timelineRows.markerLabelText
        onTrackSelected: function(trackId) { timelineRows.trackSelected(trackId) }
        onSeekRequested: function(x) { timelineRows.seekRequested(x) }
    }
}
