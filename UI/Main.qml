import QtQuick
import QtQuick.Controls
import QtQuick.Controls.Basic as Basic
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
    readonly property int markerInspectorWidth: 260
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
    readonly property color controlBandBackground: "#151922"
    readonly property color controlButtonBackground: "#242a35"
    readonly property color controlButtonHover: "#2d3442"
    readonly property color controlButtonPressed: "#353d4d"
    readonly property color controlBorder: "#3a414f"
    readonly property color controlTrack: "#2b313c"
    function createAppRuntime() {
        var component = Qt.createComponent(Qt.resolvedUrl("AppRuntime.qml"))
        if (component.status !== Component.Ready) {
            var loadError = component.errorString()
            console.error(loadError)
            throw new Error("Failed to load AppRuntime.qml: " + loadError)
        }
        var adapter = component.createObject(root)
        if (adapter === null) {
            var createError = component.errorString()
            console.error(createError.length > 0 ? createError : "Failed to create AppRuntime.qml")
            throw new Error("Failed to create AppRuntime.qml: " + createError)
        }
        return adapter
    }

    readonly property var appRuntime: typeof appController === "undefined"
        ? root.createAppRuntime()
        : null
    readonly property var controller: typeof appController === "undefined"
        ? root.appRuntime
        : appController
    readonly property string statusError: root.controller.lastError.length > 0 ? root.controller.lastError : root.controller.playback.lastError
    readonly property var markerColorOptions: root.controller.markerColorOptions

    function seekTimelineAtX(xValue) {
        root.controller.begin_timeline_user_navigation()
        root.controller.scrub_timeline_at_x(Math.max(0, xValue - root.timelineLeftPadding), timelineLaneWidth())
        root.controller.end_timeline_user_navigation()
    }

    function timelineLaneWidth() {
        var laneWidth = Math.max(0, timelineView.rowsWidth - root.timelineLabelWidth - root.timelineLeftPadding)
        return laneWidth
    }

    function updateTimelineVisibleSeconds() {
        if (typeof root.controller.set_timeline_visible_lane_width === "function") {
            root.controller.set_timeline_visible_lane_width(root.timelineLaneWidth())
        } else if (typeof root.controller.set_timeline_visible_seconds === "function") {
            root.controller.set_timeline_visible_seconds(root.timelineLaneWidth() / Math.max(1, root.controller.timelinePixelsPerSecond))
        }
    }

    function controllerNumber(propertyName, fallback) {
        if (!root.controller) return fallback
        var value = Number(root.controller[propertyName])
        return isFinite(value) ? value : fallback
    }

    function zoomSliderValueForPixels(pixelsPerSecond) {
        var minZoom = Math.max(0.001, root.controllerNumber("timelineMinPixelsPerSecond", 24))
        var maxZoom = Math.max(minZoom * 1.001, root.controllerNumber("timelineMaxPixelsPerSecond", 240))
        var currentZoom = Math.max(minZoom, Math.min(maxZoom, Number(pixelsPerSecond)))
        return (Math.log(currentZoom) - Math.log(minZoom)) / (Math.log(maxZoom) - Math.log(minZoom))
    }

    function pixelsForZoomSliderValue(value) {
        var minZoom = Math.max(0.001, root.controllerNumber("timelineMinPixelsPerSecond", 24))
        var maxZoom = Math.max(minZoom * 1.001, root.controllerNumber("timelineMaxPixelsPerSecond", 240))
        var normalized = Math.max(0, Math.min(1, Number(value)))
        return Math.exp(Math.log(minZoom) + normalized * (Math.log(maxZoom) - Math.log(minZoom)))
    }

    function setTimelineZoomForLaneWidth(pixelsPerSecond, laneWidth) {
        if (!root.controller) return
        if (typeof root.controller.set_timeline_zoom_for_lane_width === "function") {
            root.controller.set_timeline_zoom_for_lane_width(pixelsPerSecond, laneWidth)
        } else if (typeof root.controller.set_timeline_zoom === "function") {
            root.controller.set_timeline_zoom(pixelsPerSecond)
            root.updateTimelineVisibleSeconds()
        }
    }

    function fitTimelineToLaneWidth(laneWidth) {
        if (!root.controller) return
        if (typeof root.controller.fit_timeline_to_lane_width === "function") {
            root.controller.fit_timeline_to_lane_width(laneWidth)
        } else {
            root.updateTimelineVisibleSeconds()
        }
    }

    function setTimelineFollowMode(mode) {
        if (root.controller && typeof root.controller.set_timeline_follow_mode === "function") {
            root.controller.set_timeline_follow_mode(mode)
        }
    }

    function extendTimelineControlNavigation() {
        if (!root.controller) return
        if (typeof root.controller.begin_timeline_user_navigation === "function") {
            root.controller.begin_timeline_user_navigation()
            timelineControlNavigationQuietTimer.restart()
        }
    }

    function setTimelineScrollSeconds(seconds) {
        if (root.controller && typeof root.controller.set_timeline_scroll_seconds === "function") {
            root.extendTimelineControlNavigation()
            root.controller.set_timeline_scroll_seconds(seconds)
        }
    }

    function formatSeconds(seconds) {
        var safeSeconds = Math.max(0, Number(seconds))
        var minutes = Math.floor(safeSeconds / 60)
        var remaining = Math.floor(safeSeconds % 60)
        return minutes + ":" + (remaining < 10 ? "0" + remaining : remaining)
    }

    function configureTimelineSurface(item) {
        if (!item) return
        if ("appController" in item) item.appController = root.controller
        if ("timelineLeftPadding" in item) item.timelineLeftPadding = root.timelineLeftPadding
        if ("timelineLabelWidth" in item) item.timelineLabelWidth = root.timelineLabelWidth
        if ("timelineRowHeight" in item) item.timelineRowHeight = root.timelineRowHeight
        if ("timelineRulerHeight" in item) item.timelineRulerHeight = root.timelineRulerHeight
        if ("panelBackground" in item) item.panelBackground = root.panelBackground
        if ("laneBackground" in item) item.laneBackground = root.laneBackground
        if ("laneBackgroundAlt" in item) item.laneBackgroundAlt = root.laneBackgroundAlt
        if ("borderSubtle" in item) item.borderSubtle = root.borderSubtle
        if ("textPrimary" in item) item.textPrimary = root.textPrimary
        if ("textMuted" in item) item.textMuted = root.textMuted
        if ("focusAccent" in item) item.focusAccent = root.focusAccent
        if ("statusErrorColor" in item) item.statusErrorColor = root.statusErrorColor
        if ("artifactAccent" in item) item.artifactAccent = root.artifactAccent
        if ("markerLabelText" in item) item.markerLabelText = root.markerLabelText
    }

    function initializeRustRuntime() {
        root.controller.start_default_project()
        root.updateTimelineVisibleSeconds()
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

    Component.onCompleted: root.initializeRustRuntime()

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

    Timer {
        id: timelineControlNavigationQuietTimer
        interval: 220
        repeat: false
        onTriggered: {
            if (root.controller && typeof root.controller.end_timeline_user_navigation === "function") {
                root.controller.end_timeline_user_navigation()
            }
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
            onRefreshCacheRequested: root.controller.refresh_cache_status()
            onDeriveEditableRequested: root.controller.create_editable_track_from_track(root.controller.selectedTrackId)
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

        Rectangle {
            id: timelineControlBand
            Layout.fillWidth: true
            Layout.preferredHeight: 44
            color: root.controlBandBackground
            border.color: root.borderSubtle
            border.width: 1

            RowLayout {
                id: timelineControls
                anchors.fill: parent
                anchors.leftMargin: 12
                anchors.rightMargin: 12
                spacing: 10

                Label {
                    text: "ZOOM"
                    color: root.textMuted
                    font.pixelSize: 10
                    font.bold: true
                    Layout.preferredWidth: 42
                }
                Basic.Slider {
                    id: timelineZoomSlider
                    from: 0
                    to: 1
                    value: root.zoomSliderValueForPixels(root.controller.timelinePixelsPerSecond)
                    Layout.preferredWidth: 180
                    onMoved: root.setTimelineZoomForLaneWidth(
                        root.pixelsForZoomSliderValue(value),
                        root.timelineLaneWidth()
                    )
                    background: Rectangle {
                        x: timelineZoomSlider.leftPadding
                        y: timelineZoomSlider.topPadding + timelineZoomSlider.availableHeight / 2 - height / 2
                        width: timelineZoomSlider.availableWidth
                        height: 4
                        radius: 2
                        color: root.controlTrack

                        Rectangle {
                            width: timelineZoomSlider.visualPosition * parent.width
                            height: parent.height
                            radius: parent.radius
                            color: root.focusAccent
                        }
                    }
                    handle: Rectangle {
                        x: timelineZoomSlider.leftPadding + timelineZoomSlider.visualPosition * (timelineZoomSlider.availableWidth - width)
                        y: timelineZoomSlider.topPadding + timelineZoomSlider.availableHeight / 2 - height / 2
                        width: 14
                        height: 14
                        radius: 7
                        color: timelineZoomSlider.pressed ? "#fef08a" : root.focusAccent
                        border.color: "#111318"
                        border.width: 1
                    }
                }
                Basic.Button {
                    id: zoomFitButton
                    text: "Fit"
                    onClicked: root.fitTimelineToLaneWidth(root.timelineLaneWidth())
                    contentItem: Text {
                        text: zoomFitButton.text
                        color: root.textPrimary
                        font.pixelSize: 12
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                    }
                    background: Rectangle {
                        radius: 4
                        color: zoomFitButton.down ? root.controlButtonPressed : zoomFitButton.hovered ? root.controlButtonHover : root.controlButtonBackground
                        border.color: root.controlBorder
                    }
                }
                Label {
                    text: Math.round(root.controller.timelinePixelsPerSecond) + " px/s"
                    color: root.secondaryText
                    font.pixelSize: 12
                    Layout.preferredWidth: 64
                }
                Label {
                    text: "FOLLOW"
                    color: root.textMuted
                    font.pixelSize: 10
                    font.bold: true
                    Layout.preferredWidth: 54
                }
                Basic.ComboBox {
                    id: followModeSelector
                    model: [
                        { text: "Off", value: 0 },
                        { text: "Band", value: 1 },
                        { text: "Center", value: 2 }
                    ]
                    textRole: "text"
                    valueRole: "value"
                    currentIndex: Math.max(0, Math.min(2, root.controllerNumber("timelineFollowMode", 0)))
                    Layout.preferredWidth: 112
                    onActivated: root.setTimelineFollowMode(currentValue)
                    contentItem: Text {
                        text: followModeSelector.displayText
                        color: root.textPrimary
                        font.pixelSize: 12
                        verticalAlignment: Text.AlignVCenter
                        leftPadding: 10
                        rightPadding: 24
                    }
                    background: Rectangle {
                        radius: 4
                        color: followModeSelector.down ? root.controlButtonPressed : followModeSelector.hovered ? root.controlButtonHover : root.controlButtonBackground
                        border.color: root.controlBorder
                    }
                }
                Label {
                    text: "SCROLL"
                    color: root.textMuted
                    font.pixelSize: 10
                    font.bold: true
                    Layout.preferredWidth: 48
                }
                Basic.Slider {
                    id: timelineScrollSlider
                    from: 0
                    to: Math.max(0, root.controller.timelineDurationSeconds - root.controller.timelineVisibleSeconds)
                    value: root.controller.timelineScrollSeconds
                    Layout.fillWidth: true
                    onMoved: root.setTimelineScrollSeconds(value)
                    background: Rectangle {
                        x: timelineScrollSlider.leftPadding
                        y: timelineScrollSlider.topPadding + timelineScrollSlider.availableHeight / 2 - height / 2
                        width: timelineScrollSlider.availableWidth
                        height: 4
                        radius: 2
                        color: root.controlTrack

                        Rectangle {
                            width: timelineScrollSlider.visualPosition * parent.width
                            height: parent.height
                            radius: parent.radius
                            color: root.artifactAccent
                        }
                    }
                    handle: Rectangle {
                        x: timelineScrollSlider.leftPadding + timelineScrollSlider.visualPosition * (timelineScrollSlider.availableWidth - width)
                        y: timelineScrollSlider.topPadding + timelineScrollSlider.availableHeight / 2 - height / 2
                        width: 14
                        height: 14
                        radius: 7
                        color: timelineScrollSlider.pressed ? "#bfdbfe" : root.artifactAccent
                        border.color: "#111318"
                        border.width: 1
                    }
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            Loader {
                id: timelineView
                property real rowsWidth: item && "rowsWidth" in item ? item.rowsWidth : width
                signal layoutWidthChanged()
                signal trackSelected(string trackId)
                signal seekRequested(real x)
                source: "components/TimelineView.qml"
                Layout.fillWidth: true
                Layout.fillHeight: true
                onWidthChanged: timelineView.layoutWidthChanged()
                onLayoutWidthChanged: root.updateTimelineVisibleSeconds()
                onTrackSelected: function(trackId) { root.controller.select_track(trackId) }
                onSeekRequested: function(x) { root.seekTimelineAtX(x) }
                onLoaded: {
                    root.configureTimelineSurface(item)
                    if (item.layoutWidthChanged) {
                        item.layoutWidthChanged.connect(function() {
                            timelineView.layoutWidthChanged()
                        })
                    }
                    if (item.trackSelected) {
                        item.trackSelected.connect(function(trackId) {
                            timelineView.trackSelected(trackId)
                        })
                    }
                    if (item.seekRequested) {
                        item.seekRequested.connect(function(x) {
                            timelineView.seekRequested(x)
                        })
                    }
                    timelineView.layoutWidthChanged()
                }
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
                Layout.preferredWidth: root.markerInspectorWidth
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
