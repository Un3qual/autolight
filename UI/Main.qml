import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import "components"

Window {
    id: root
    width: 1120
    height: 720
    visible: true
    title: root.controller.projectName
    color: "#181a1f"
    readonly property real timelineLeftPadding: 24
    readonly property real timelineLabelWidth: 280
    readonly property real timelineRulerHeight: 32
    readonly property int timelineRowHeight: 76
    readonly property int compactButtonHeight: 30
    readonly property real defaultMarkerDuration: 8.0
    readonly property real defaultMarkerInterval: 0.5
    readonly property color panelBackground: "#1c1f26"
    readonly property color laneBackground: "#171a20"
    readonly property color laneBackgroundAlt: "#14171d"
    readonly property color borderSubtle: "#2f333d"
    readonly property color textPrimary: "#f4f4f5"
    readonly property color textMuted: "#a1a1aa"
    readonly property color focusAccent: "#facc15"
    readonly property color toolbarForeground: "#111318"
    readonly property color secondaryText: "#d4d4d8"
    readonly property color markerLabelText: "#111318"
    readonly property color selectedMarkerBackground: "#2f4366"
    readonly property color statusErrorColor: "#f87171"
    readonly property color artifactAccent: "#93c5fd"
    readonly property color footerBackground: "#111318"
    readonly property color controlTextColor: root.textPrimary
    readonly property color controlMutedTextColor: root.textMuted
    function createRustAdapter() {
        var component = Qt.createComponent(Qt.resolvedUrl("RustAdapter.qml"))
        if (component.status !== Component.Ready) {
            console.error(component.errorString())
            return null
        }
        return component.createObject(root)
    }

    readonly property var rustAdapter: typeof appController === "undefined"
        ? root.createRustAdapter()
        : null
    readonly property var controller: typeof appController === "undefined"
        ? root.rustAdapter
        : appController
    readonly property string statusError: root.controller.lastError.length > 0 ? root.controller.lastError : root.controller.playback.lastError
    readonly property var markerColorOptions: root.controller.markerColorOptions

    function seekTimelineAtX(xValue) {
        var laneSeconds = root.controller.timelineScrollSeconds
            + Math.max(0, xValue - root.timelineLeftPadding) / root.controller.timelinePixelsPerSecond
        root.controller.seek_playback(Math.min(root.controller.timelineDurationSeconds, laneSeconds))
    }

    function updateTimelineVisibleSeconds() {
        var laneWidth = Math.max(0, timelineView.rowsWidth - root.timelineLabelWidth - root.timelineLeftPadding)
        root.controller.set_timeline_visible_seconds(laneWidth / root.controller.timelinePixelsPerSecond)
    }

    function formatSeconds(seconds) {
        var safeSeconds = Math.max(0, Number(seconds))
        var minutes = Math.floor(safeSeconds / 60)
        var remaining = Math.floor(safeSeconds % 60)
        return minutes + ":" + (remaining < 10 ? "0" + remaining : remaining)
    }

    function togglePlayback() {
        if (root.controller.playback.isPlaying) {
            root.controller.pause_playback()
        } else if (root.controller.selectedTrackId.length === 0 && root.controller.playback.sourcePath.length > 0) {
            root.controller.playback.play()
        } else {
            root.controller.play_selected_track()
        }
    }

    function newProjectWithConfirmation() {
        if (root.controller.isDirty) {
            discardChangesDialog.pendingAction = "new"
            discardChangesDialog.pendingPath = ""
            discardChangesDialog.open()
        } else {
            root.controller.new_project()
        }
    }

    function openProjectWithConfirmation(path) {
        if (root.controller.isDirty) {
            discardChangesDialog.pendingAction = "open"
            discardChangesDialog.pendingPath = path
            discardChangesDialog.open()
        } else {
            root.controller.open_project(path)
        }
    }

    function demoProjectWithConfirmation() {
        if (root.controller.isDirty) {
            discardChangesDialog.pendingAction = "demo"
            discardChangesDialog.pendingPath = ""
            discardChangesDialog.open()
        } else {
            root.controller.load_demo_project()
        }
    }

    function runPendingDiscardAction() {
        if (discardChangesDialog.pendingAction === "new") {
            root.controller.new_project()
        } else if (discardChangesDialog.pendingAction === "open") {
            root.controller.open_project(discardChangesDialog.pendingPath)
        } else if (discardChangesDialog.pendingAction === "demo") {
            root.controller.load_demo_project()
        }
        discardChangesDialog.pendingAction = ""
        discardChangesDialog.pendingPath = ""
    }

    Component.onCompleted: root.updateTimelineVisibleSeconds()

    Connections {
        target: root.controller
        function onTimelinePixelsPerSecondChanged() {
            root.updateTimelineVisibleSeconds()
        }
    }

    FileDialog {
        id: openProjectDialog
        title: "Open Autolight Project"
        nameFilters: ["Autolight projects (*.autolight)"]
        fileMode: FileDialog.OpenFile
        onAccepted: root.openProjectWithConfirmation(String(selectedFile))
    }

    FileDialog {
        id: saveProjectDialog
        title: "Save Autolight Project"
        nameFilters: ["Autolight projects (*.autolight)"]
        fileMode: FileDialog.SaveFile
        onAccepted: root.controller.save_project(String(selectedFile))
    }

    FileDialog {
        id: importAudioDialog
        title: "Import Audio"
        nameFilters: ["WAV audio files (*.wav)", "All files (*)"]
        fileMode: FileDialog.OpenFile
        onAccepted: root.controller.import_audio(String(selectedFile))
    }

    Dialog {
        id: discardChangesDialog
        title: "Discard unsaved changes?"
        modal: true
        width: 420
        anchors.centerIn: parent
        property string pendingAction: ""
        property string pendingPath: ""

        contentItem: Text {
            text: "This project has unsaved changes."
            color: root.textPrimary
            wrapMode: Text.WordWrap
            font.pixelSize: 13
        }

        footer: DialogButtonBox {
            Button { text: "Cancel"; DialogButtonBox.buttonRole: DialogButtonBox.RejectRole }
            Button { text: "Discard"; DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole }
        }

        onAccepted: root.runPendingDiscardAction()
        onRejected: {
            pendingAction = ""
            pendingPath = ""
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        ProjectToolbar {
            appController: root.controller
            compactButtonHeight: root.compactButtonHeight
            toolbarForeground: root.toolbarForeground
            Layout.fillWidth: true
            onNewRequested: root.newProjectWithConfirmation()
            onOpenRequested: openProjectDialog.open()
            onSaveAsRequested: saveProjectDialog.open()
            onDemoRequested: root.demoProjectWithConfirmation()
            onImportAudioRequested: importAudioDialog.open()
        }

        TransformBar {
            appController: root.controller
            compactButtonHeight: root.compactButtonHeight
            controlTextColor: root.controlTextColor
            controlMutedTextColor: root.controlMutedTextColor
            secondaryText: root.secondaryText
            textMuted: root.textMuted
            Layout.fillWidth: true
            onAddMarkersRequested: root.controller.add_fixed_interval_track(root.controller.selectedTrackId, root.defaultMarkerDuration, root.defaultMarkerInterval)
            onRunRequested: root.controller.run_track(root.controller.selectedTrackId)
            onRerunRequested: root.controller.rerun_track(root.controller.selectedTrackId)
            onCancelRequested: root.controller.cancel_selected_job()
            onAddTransformRequested: function(transformId, transformVersion, params) {
                root.controller.add_transform_track(root.controller.selectedTrackId, transformId, transformVersion, params)
            }
            onAddVocalsStemRequested: root.controller.add_vocals_stem_track(root.controller.selectedTrackId)
            onRefreshCacheRequested: root.controller.refresh_cache_status()
            onDeriveEditableRequested: root.controller.create_editable_track_from_track(root.controller.selectedTrackId)
        }

        TimelineRuler {
            appController: root.controller
            timelineLeftPadding: root.timelineLeftPadding
            timelineLabelWidth: root.timelineLabelWidth
            timelineRulerHeight: root.timelineRulerHeight
            panelBackground: root.panelBackground
            borderSubtle: root.borderSubtle
            textMuted: root.textMuted
            Layout.fillWidth: true
        }

        PlaybackBar {
            appController: root.controller
            controlTextColor: root.controlTextColor
            secondaryText: root.secondaryText
            formatSeconds: root.formatSeconds
            Layout.fillWidth: true
            onNudgeRequested: function(delta) { root.controller.nudge_playback(delta) }
            onTogglePlaybackRequested: root.togglePlayback()
            onStopRequested: root.controller.stop_playback()
            onVolumeRequested: function(value) { root.controller.playback.set_volume(value) }
            onSeekRequested: function(value) { root.controller.seek_playback(value) }
        }

        RowLayout {
            id: timelineControls
            Layout.fillWidth: true
            Layout.leftMargin: 12
            Layout.rightMargin: 12
            spacing: 10

            Label { text: "Zoom"; color: root.secondaryText; font.pixelSize: 12 }
            Slider {
                id: timelineZoomSlider
                from: 24
                to: 240
                value: root.controller.timelinePixelsPerSecond
                Layout.preferredWidth: 180
                onMoved: root.controller.set_timeline_zoom(value)
            }
            Label {
                text: Math.round(root.controller.timelinePixelsPerSecond) + " px/s"
                color: root.textMuted
                font.pixelSize: 12
                Layout.preferredWidth: 64
            }
            Slider {
                id: timelineScrollSlider
                from: 0
                to: Math.max(0, root.controller.timelineDurationSeconds - root.controller.timelineVisibleSeconds)
                value: root.controller.timelineScrollSeconds
                Layout.fillWidth: true
                onMoved: root.controller.set_timeline_scroll_seconds(value)
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            TimelineView {
                id: timelineView
                appController: root.controller
                timelineLeftPadding: root.timelineLeftPadding
                timelineLabelWidth: root.timelineLabelWidth
                timelineRowHeight: root.timelineRowHeight
                panelBackground: root.panelBackground
                laneBackground: root.laneBackground
                laneBackgroundAlt: root.laneBackgroundAlt
                borderSubtle: root.borderSubtle
                textPrimary: root.textPrimary
                textMuted: root.textMuted
                focusAccent: root.focusAccent
                statusErrorColor: root.statusErrorColor
                artifactAccent: root.artifactAccent
                markerLabelText: root.markerLabelText
                Layout.fillWidth: true
                Layout.fillHeight: true
                onLayoutWidthChanged: root.updateTimelineVisibleSeconds()
                onTrackSelected: function(trackId) { root.controller.select_track(trackId) }
                onSeekRequested: function(x) { root.seekTimelineAtX(x) }
            }

            MarkerInspector {
                id: markerInspector
                appController: root.controller
                markerColorOptions: root.markerColorOptions
                panelBackground: root.panelBackground
                borderSubtle: root.borderSubtle
                textPrimary: root.textPrimary
                textMuted: root.textMuted
                selectedMarkerBackground: root.selectedMarkerBackground
                Layout.preferredWidth: 260
                Layout.fillHeight: true
                onAddCueRequested: function(timestamp, duration, label, category, colorKey) {
                    root.controller.add_marker_to_selected_track_with_duration(timestamp, duration, label, category, colorKey)
                }
                onDeleteCueRequested: function(markerId) {
                    if (root.controller.delete_marker_from_selected_track(markerId)) {
                        markerInspector.clearSelectionId()
                    }
                }
                onDeleteSelectedCuesRequested: {
                    if (root.controller.delete_selected_markers() > 0) {
                        markerInspector.clearSelectionId()
                    }
                }
                onUpdateCueRequested: function(timestamp, duration, label, category, colorKey) {
                    root.controller.update_selected_marker_with_duration(timestamp, duration, label, category, colorKey)
                }
                onBulkUpdateRequested: function(label, category, colorKey) {
                    root.controller.bulk_update_selected_markers(label, category, colorKey)
                }
                onToggleMarkerSelectionRequested: function(markerId, extendSelection) {
                    root.controller.toggle_marker_selection(markerId, extendSelection)
                    markerInspector.syncMarkerEditorFromSelection()
                }
            }
        }

        StatusFooter {
            appController: root.controller
            statusError: root.statusError
            footerBackground: root.footerBackground
            borderSubtle: root.borderSubtle
            statusErrorColor: root.statusErrorColor
            textMuted: root.textMuted
            Layout.fillWidth: true
        }
    }
}
