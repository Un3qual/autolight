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
    readonly property string rustAdapterSource: [
        "import QtQml",
        "import QtQml.Models",
        "import QtMultimedia",
        "import Autolight.Qt 1.0",
        "QtObject {",
        "    id: rustAdapter",
        "    property var rustController: AppController {}",
        "    readonly property string projectName: rustController.projectName",
        "    readonly property string lastError: rustController.lastError",
        "    readonly property string timelineRowsJson: rustController.timelineRowsJson",
        "    readonly property string transformSpecsJson: rustController.transformSpecsJson",
        "    readonly property string selectedMarkerIdsJson: rustController.selectedMarkerIdsJson",
        "    readonly property string selectedTrackMarkersJson: rustController.selectedTrackMarkersJson",
        "    readonly property string markerColorOptionsJson: rustController.markerColorOptionsJson",
        "    readonly property string projectPath: rustController.projectPath",
        "    readonly property bool isDirty: rustController.isDirty",
        "    readonly property bool canUndo: rustController.canUndo",
        "    readonly property bool canRedo: rustController.canRedo",
        "    readonly property string selectedTrackId: rustController.selectedTrackId",
        "    readonly property bool selectedTrackCanPlay: rustController.selectedTrackCanPlay",
        "    readonly property bool selectedTrackCanRerun: rustController.selectedTrackCanRerun",
        "    readonly property bool selectedTrackHasRunningJob: rustController.selectedTrackHasRunningJob",
        "    readonly property bool selectedTrackIsEditable: rustController.selectedTrackIsEditable",
        "    property var selectedMarkerIds: []",
        "    property var selectedTrackMarkers: []",
        "    property var markerColorOptions: []",
        "    readonly property real timelinePixelsPerSecond: rustController.timelinePixelsPerSecond",
        "    readonly property real timelineScrollSeconds: rustController.timelineScrollSeconds",
        "    readonly property real timelineVisibleSeconds: rustController.timelineVisibleSeconds",
        "    readonly property real timelineDurationSeconds: rustController.timelineDurationSeconds",
        "    property var trackModel: ListModel {}",
        "    property var transformModel: ListModel {",
        "        function version_at(index) {",
        "            if (index < 0 || index >= count) return \"\"",
        "            return get(index).version",
        "        }",
        "    }",
        "    property var audioOutput: AudioOutput { volume: rustController.playbackVolume }",
        "    property var mediaPlayer: MediaPlayer {",
        "        audioOutput: rustAdapter.audioOutput",
        "    }",
        "    property var playback: QtObject {",
        "        readonly property string lastError: rustController.playbackLastError.length > 0 ? rustController.playbackLastError : rustAdapter.mediaPlayer.errorString",
        "        readonly property bool isPlaying: rustAdapter.mediaPlayer.playbackState === MediaPlayer.PlayingState",
        "        readonly property string sourcePath: rustController.playbackSourcePath",
        "        readonly property real positionSeconds: rustAdapter.mediaPlayer.source.toString().length > 0 ? rustAdapter.mediaPlayer.position / 1000.0 : rustController.playbackPositionSeconds",
        "        readonly property real durationSeconds: rustAdapter.mediaPlayer.duration > 0 ? rustAdapter.mediaPlayer.duration / 1000.0 : rustController.playbackDurationSeconds",
        "        readonly property real volume: rustAdapter.audioOutput.volume",
        "        function play() {",
        "            var played = rustController.playLoadedPlayback()",
        "            rustAdapter.reloadModels()",
        "            if (played && rustAdapter.syncPlaybackSource()) rustAdapter.mediaPlayer.play()",
        "            return played",
        "        }",
        "        function set_volume(value) {",
        "            rustController.setPlaybackVolumeValue(value)",
        "            rustAdapter.audioOutput.volume = rustController.playbackVolume",
        "            rustAdapter.reloadModels()",
        "        }",
        "    }",
        "    function playbackSourceUrl(path) {",
        "        if (path.length === 0) return \"\"",
        "        if (path.indexOf(\"file://\") === 0) return path",
        "        return \"file://\" + encodeURI(path)",
        "    }",
        "    function syncPlaybackSource() {",
        "        var path = rustController.playbackSourcePath",
        "        if (path.length === 0) {",
        "            mediaPlayer.stop()",
        "            mediaPlayer.source = \"\"",
        "            return false",
        "        }",
        "        var sourceUrl = playbackSourceUrl(path)",
        "        if (mediaPlayer.source.toString() !== sourceUrl) mediaPlayer.source = sourceUrl",
        "        return true",
        "    }",
        "    function reloadTrackModel() {",
        "        trackModel.clear()",
        "        var rows = []",
        "        try { rows = JSON.parse(rustController.timelineRowsJson) } catch (error) { return }",
        "        for (var i = 0; i < rows.length; i++) {",
        "            trackModel.append(rows[i])",
        "        }",
        "    }",
        "    function parseJsonArray(payload) {",
        "        try {",
        "            var rows = JSON.parse(payload)",
        "            return Array.isArray(rows) ? rows : []",
        "        } catch (error) {",
        "            return []",
        "        }",
        "    }",
        "    function reloadTransformModel() {",
        "        transformModel.clear()",
        "        var rows = []",
        "        try { rows = JSON.parse(rustController.transformSpecsJson) } catch (error) { return }",
        "        for (var i = 0; i < rows.length; i++) {",
        "            transformModel.append(rows[i])",
        "        }",
        "    }",
        "    function reloadSelectionModels() {",
        "        selectedMarkerIds = parseJsonArray(rustController.selectedMarkerIdsJson)",
        "        selectedTrackMarkers = parseJsonArray(rustController.selectedTrackMarkersJson)",
        "        markerColorOptions = parseJsonArray(rustController.markerColorOptionsJson)",
        "    }",
        "    function reloadModels() { reloadTrackModel(); reloadTransformModel(); reloadSelectionModels(); syncPlaybackSource(); audioOutput.volume = rustController.playbackVolume }",
        "    function new_project() { rustController.newProject(); reloadModels() }",
        "    function open_project(path) { var opened = rustController.openProject(path); reloadModels(); return opened }",
        "    function save_project(path) { var saved = rustController.saveProject(path || \"\"); reloadModels(); return saved }",
        "    function import_audio(path) { var id = rustController.importAudio(path); reloadModels(); return id }",
        "    function load_demo_project() { rustController.loadDemoProject(); reloadModels() }",
        "    function add_manual_cue_track(name) { var id = rustController.addManualCueTrack(name || \"Manual Cues\"); reloadModels(); return id }",
        "    function undo() { var changed = rustController.undo(); reloadModels(); return changed }",
        "    function redo() { var changed = rustController.redo(); reloadModels(); return changed }",
        "    function add_fixed_interval_track(trackId, duration, interval) { return add_transform_track(trackId, \"markers.fixed_interval\", \"1\", JSON.stringify({\"duration\": duration, \"interval\": interval})) }",
        "    function run_track(trackId) { var id = rustController.runTrack(trackId); reloadModels(); return id }",
        "    function rerun_track(trackId) { var id = rustController.rerunTrack(trackId); reloadModels(); return id }",
        "    function cancel_selected_job() { rustController.cancelSelectedJob(); reloadModels() }",
        "    function add_transform_track(trackId, transformId, transformVersion, params) { var id = rustController.addTransformTrack(trackId, transformId, transformVersion, params); reloadModels(); return id }",
        "    function add_vocals_stem_track(trackId) { return add_transform_track(trackId, \"stems.vocals_stand_in\", \"1\", \"{}\") }",
        "    function refresh_cache_status() { var refs = rustController.refreshCacheStatus(); reloadModels(); return refs }",
        "    function create_editable_track_from_track(trackId) { var id = rustController.createEditableTrackFromTrack(trackId); reloadModels(); return id }",
        "    function pause_playback() { rustController.pausePlayback(); mediaPlayer.pause(); reloadModels() }",
        "    function play_selected_track() { var played = rustController.playSelectedTrack(); reloadModels(); if (played && syncPlaybackSource()) mediaPlayer.play(); return played }",
        "    function stop_playback() { rustController.stopPlayback(); mediaPlayer.stop(); mediaPlayer.seek(0); reloadModels() }",
        "    function nudge_playback(delta) { seek_playback(playback.positionSeconds + delta) }",
        "    function seek_playback(value) { rustController.seekPlayback(value); reloadModels(); if (syncPlaybackSource()) mediaPlayer.seek(rustController.playbackPositionSeconds * 1000) }",
        "    function set_timeline_zoom(value) { rustController.setTimelineZoom(value); reloadModels() }",
        "    function set_timeline_scroll_seconds(value) { rustController.applyTimelineScrollSeconds(value); reloadModels() }",
        "    function set_timeline_visible_seconds(value) { rustController.applyTimelineVisibleSeconds(value); reloadModels() }",
        "    function set_timeline_visible_track_range(firstRow, rowCount) { rustController.setTimelineVisibleTrackRange(firstRow, rowCount); reloadModels() }",
        "    function select_track(trackId) { rustController.selectTrack(trackId); reloadModels() }",
        "    function set_track_expanded(trackId, expanded) { var changed = rustController.setTrackExpanded(trackId, expanded); reloadModels(); return changed }",
        "    function snap_timeline_time(seconds, bypassSnap) { return rustController.snapTimelineTime(seconds, bypassSnap) }",
        "    function add_marker_to_selected_track_with_duration(timestamp, duration, label, category, colorKey) { var id = rustController.addMarkerToSelectedTrackWithDuration(timestamp, duration, label, category, colorKey); reloadModels(); return id }",
        "    function delete_marker_from_selected_track(markerId) { var deleted = rustController.deleteMarkerFromSelectedTrack(markerId); reloadModels(); return deleted }",
        "    function delete_selected_markers() { var deleted = rustController.deleteSelectedMarkers(); reloadModels(); return deleted }",
        "    function update_selected_marker_with_duration(timestamp, duration, label, category, colorKey) { var updated = rustController.updateSelectedMarkerWithDuration(timestamp, duration, label, category, colorKey); reloadModels(); return updated }",
        "    function bulk_update_selected_markers(label, category, colorKey) { var updated = rustController.bulkUpdateSelectedMarkers(label, category, colorKey); reloadModels(); return updated }",
        "    function toggle_marker_selection(markerId, extendSelection) { rustController.toggleMarkerSelection(markerId, extendSelection); reloadModels() }",
        "    function move_selected_markers(delta, bypass) { var moved = rustController.moveSelectedMarkers(delta, bypass); reloadModels(); return moved }",
        "    function resize_marker(markerId, duration) { var resized = rustController.resizeMarker(markerId, duration); reloadModels(); return resized }",
        "    Component.onCompleted: load_demo_project()",
        "}",
    ].join("\n")
    readonly property var controller: typeof appController === "undefined"
        ? Qt.createQmlObject(root.rustAdapterSource, root, "RustAppControllerAdapter")
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
        nameFilters: ["Audio files (*.wav *.mp3 *.flac *.aiff *.aif *.m4a)", "All files (*)"]
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
