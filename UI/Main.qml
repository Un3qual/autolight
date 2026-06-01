import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts

Window {
    id: root
    width: 1120
    height: 720
    visible: true
    title: appController.projectName
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
    readonly property string statusError: appController.lastError.length > 0 ? appController.lastError : appController.playback.lastError
    readonly property var markerColorOptions: appController.markerColorOptions

    function timelineX(seconds) {
        return root.timelineLeftPadding + (seconds - appController.timelineScrollSeconds) * appController.timelinePixelsPerSecond
    }

    function seekTimelineAtX(xValue) {
        var laneSeconds = appController.timelineScrollSeconds
            + Math.max(0, xValue - root.timelineLeftPadding) / appController.timelinePixelsPerSecond
        appController.seek_playback(Math.min(appController.timelineDurationSeconds, laneSeconds))
    }

    function markerColorIndex(colorKey) {
        for (var i = 0; i < root.markerColorOptions.length; i++) {
            if (root.markerColorOptions[i].key === colorKey) {
                return i
            }
        }
        return 0
    }

    function selectedMarkerCount() {
        return appController.selectedMarkerIds.length
    }

    function syncMarkerEditor(marker) {
        markerTimestampField.text = Number(marker.timestamp).toFixed(2)
        markerLabelField.text = marker.label.length > 0 ? marker.label : "Cue"
        markerCategoryField.text = marker.category.length > 0 ? marker.category : "cue"
        markerColorPicker.currentIndex = root.markerColorIndex(marker.colorKey)
    }

    function syncMarkerEditorFromSelection() {
        inspectorPanel.selectedMarkerId = ""
        if (appController.selectedMarkerIds.length !== 1) {
            return
        }

        var markerId = appController.selectedMarkerIds[0]
        for (var i = 0; i < appController.selectedTrackMarkers.length; i++) {
            var marker = appController.selectedTrackMarkers[i]
            if (marker.id === markerId && marker.selected) {
                inspectorPanel.selectedMarkerId = marker.id
                root.syncMarkerEditor(marker)
                return
            }
        }
    }

    function updateTimelineVisibleSeconds() {
        var laneWidth = Math.max(0, timelineRows.width - root.timelineLabelWidth - root.timelineLeftPadding)
        appController.set_timeline_visible_seconds(laneWidth / appController.timelinePixelsPerSecond)
    }

    function formatSeconds(seconds) {
        var safeSeconds = Math.max(0, Number(seconds))
        var minutes = Math.floor(safeSeconds / 60)
        var remaining = Math.floor(safeSeconds % 60)
        return minutes + ":" + (remaining < 10 ? "0" + remaining : remaining)
    }

    function togglePlayback() {
        if (appController.playback.isPlaying) {
            appController.pause_playback()
        } else if (appController.selectedTrackId.length === 0 && appController.playback.sourcePath.length > 0) {
            appController.playback.play()
        } else {
            appController.play_selected_track()
        }
    }

    function newProjectWithConfirmation() {
        if (appController.isDirty) {
            discardChangesDialog.pendingAction = "new"
            discardChangesDialog.pendingPath = ""
            discardChangesDialog.open()
        } else {
            appController.new_project()
        }
    }

    function openProjectWithConfirmation(path) {
        if (appController.isDirty) {
            discardChangesDialog.pendingAction = "open"
            discardChangesDialog.pendingPath = path
            discardChangesDialog.open()
        } else {
            appController.open_project(path)
        }
    }

    function demoProjectWithConfirmation() {
        if (appController.isDirty) {
            discardChangesDialog.pendingAction = "demo"
            discardChangesDialog.pendingPath = ""
            discardChangesDialog.open()
        } else {
            appController.load_demo_project()
        }
    }

    function runPendingDiscardAction() {
        if (discardChangesDialog.pendingAction === "new") {
            appController.new_project()
        } else if (discardChangesDialog.pendingAction === "open") {
            appController.open_project(discardChangesDialog.pendingPath)
        } else if (discardChangesDialog.pendingAction === "demo") {
            appController.load_demo_project()
        }
        discardChangesDialog.pendingAction = ""
        discardChangesDialog.pendingPath = ""
    }

    Component.onCompleted: root.updateTimelineVisibleSeconds()

    Connections {
        target: appController
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
        onAccepted: appController.save_project(String(selectedFile))
    }

    FileDialog {
        id: importAudioDialog
        title: "Import Audio"
        nameFilters: ["Audio files (*.wav *.mp3 *.flac *.aiff *.aif *.m4a)", "All files (*)"]
        fileMode: FileDialog.OpenFile
        onAccepted: appController.import_audio(String(selectedFile))
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
            Button {
                text: "Cancel"
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
            Button {
                text: "Discard"
                DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
            }
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

        ToolBar {
            Layout.fillWidth: true

            RowLayout {
                anchors.fill: parent
                spacing: 8

                Label {
                    text: appController.projectName
                    color: root.toolbarForeground
                    font.pixelSize: 16
                    font.bold: true
                    Layout.leftMargin: 12
                }

                Item { Layout.fillWidth: true }

                RowLayout {
                    id: fileActions
                    spacing: 6

                    Button {
                        text: "New"
                        implicitHeight: root.compactButtonHeight
                        onClicked: root.newProjectWithConfirmation()
                    }

                    Button {
                        text: "Open"
                        implicitHeight: root.compactButtonHeight
                        onClicked: openProjectDialog.open()
                    }

                    Button {
                        text: "Save"
                        implicitHeight: root.compactButtonHeight
                        onClicked: appController.projectPath.length > 0 ? appController.save_project("") : saveProjectDialog.open()
                    }

                    Button {
                        text: "Save As"
                        implicitHeight: root.compactButtonHeight
                        onClicked: saveProjectDialog.open()
                    }

                    Button {
                        text: "Demo"
                        implicitHeight: root.compactButtonHeight
                        onClicked: root.demoProjectWithConfirmation()
                    }
                }

                RowLayout {
                    id: transformActions
                    spacing: 6

                    Button {
                        text: "Import Audio"
                        implicitHeight: root.compactButtonHeight
                        onClicked: importAudioDialog.open()
                    }

                    Button {
                        text: "Add Markers"
                        implicitHeight: root.compactButtonHeight
                        enabled: appController.selectedTrackId.length > 0
                        onClicked: appController.add_fixed_interval_track(appController.selectedTrackId, root.defaultMarkerDuration, root.defaultMarkerInterval)
                    }

                    Button {
                        text: "Run"
                        implicitHeight: root.compactButtonHeight
                        enabled: appController.selectedTrackCanRerun && !appController.selectedTrackHasRunningJob
                        onClicked: appController.run_track(appController.selectedTrackId)
                    }

                    Button {
                        text: "Rerun"
                        implicitHeight: root.compactButtonHeight
                        enabled: appController.selectedTrackCanRerun && !appController.selectedTrackHasRunningJob
                        onClicked: appController.rerun_track(appController.selectedTrackId)
                    }

                    Button {
                        text: "Cancel"
                        implicitHeight: root.compactButtonHeight
                        enabled: appController.selectedTrackHasRunningJob
                        onClicked: appController.cancel_selected_job()
                    }
                }
            }
        }

        RowLayout {
            id: timelineRuler
            Layout.fillWidth: true
            Layout.minimumHeight: root.timelineRulerHeight
            Layout.preferredHeight: root.timelineRulerHeight
            Layout.maximumHeight: root.timelineRulerHeight
            spacing: 0

            Rectangle {
                Layout.preferredWidth: root.timelineLabelWidth
                Layout.fillHeight: true
                color: root.panelBackground
                border.color: root.borderSubtle
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                color: root.panelBackground

                Repeater {
                    model: Math.ceil(appController.timelineVisibleSeconds) + 1
                    Text {
                        property real tickSecond: Math.ceil(appController.timelineScrollSeconds) + index
                        x: root.timelineX(tickSecond)
                        y: 9
                        text: tickSecond + "s"
                        color: root.textMuted
                        font.pixelSize: 12
                    }
                }
            }
        }

        RowLayout {
            id: transformDetailBar
            Layout.fillWidth: true
            Layout.leftMargin: 12
            Layout.rightMargin: 12
            Layout.topMargin: 6
            Layout.bottomMargin: 6
            spacing: 8

            ComboBox {
                id: transformPicker
                model: appController.transformModel
                textRole: "name"
                valueRole: "transformId"
                Layout.preferredWidth: 190
                palette.text: root.controlTextColor
                palette.buttonText: root.controlTextColor
            }

            TextField {
                id: transformParamsField
                text: "{\"duration\": 8.0, \"interval\": 0.5}"
                placeholderText: "JSON params"
                Layout.preferredWidth: 210
                color: root.controlTextColor
                placeholderTextColor: root.controlMutedTextColor
                selectedTextColor: root.controlTextColor
                selectionColor: "#2563eb"
            }

            Button {
                text: "Add Transform"
                palette.buttonText: root.controlTextColor
                enabled: appController.selectedTrackId.length > 0 && transformPicker.currentIndex >= 0
                onClicked: appController.add_transform_track(
                    appController.selectedTrackId,
                    transformPicker.currentValue,
                    appController.transformModel.version_at(transformPicker.currentIndex),
                    transformParamsField.text
                )
            }

            Button {
                text: "Add Vocals Stem"
                palette.buttonText: root.controlTextColor
                enabled: appController.selectedTrackId.length > 0
                onClicked: appController.add_vocals_stem_track(appController.selectedTrackId)
            }

            Button {
                text: "Check Cache"
                palette.buttonText: root.controlTextColor
                onClicked: appController.refresh_cache_status()
            }

            Button {
                text: "Derive Editable"
                palette.buttonText: root.controlTextColor
                enabled: appController.selectedTrackId.length > 0
                onClicked: appController.create_editable_track_from_track(appController.selectedTrackId)
            }

            Item { Layout.fillWidth: true }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 12
            Layout.rightMargin: 12
            spacing: 8

            RowLayout {
                id: playbackControls
                spacing: 6

                Button {
                    text: "-1s"
                    palette.buttonText: root.controlTextColor
                    enabled: appController.playback.sourcePath.length > 0
                    onClicked: appController.nudge_playback(-1.0)
                }

                Button {
                    text: appController.playback.isPlaying ? "Pause" : "Play"
                    palette.buttonText: root.controlTextColor
                    enabled: appController.selectedTrackCanPlay || (appController.selectedTrackId.length === 0 && appController.playback.sourcePath.length > 0) || appController.playback.isPlaying
                    onClicked: root.togglePlayback()
                }

                Button {
                    text: "Stop"
                    palette.buttonText: root.controlTextColor
                    enabled: appController.playback.sourcePath.length > 0
                    onClicked: appController.stop_playback()
                }

                Button {
                    text: "+1s"
                    palette.buttonText: root.controlTextColor
                    enabled: appController.playback.sourcePath.length > 0
                    onClicked: appController.nudge_playback(1.0)
                }

                Slider {
                    id: playbackVolumeSlider
                    from: 0
                    to: 1
                    value: appController.playback.volume
                    Layout.preferredWidth: 88
                    onMoved: appController.playback.set_volume(value)
                }

                Label {
                    id: playheadTimeLabel
                    text: root.formatSeconds(appController.playback.positionSeconds) + " / " + root.formatSeconds(appController.playback.durationSeconds)
                    color: root.secondaryText
                    font.pixelSize: 12
                }
            }

            Slider {
                id: playbackScrubber
                Layout.fillWidth: true
                from: 0
                to: Math.max(0.01, appController.playback.durationSeconds)
                value: appController.playback.positionSeconds
                enabled: appController.playback.sourcePath.length > 0
                live: true
                onMoved: appController.seek_playback(value)
            }
        }

        RowLayout {
            id: timelineControls
            Layout.fillWidth: true
            Layout.leftMargin: 12
            Layout.rightMargin: 12
            spacing: 10

            Label {
                text: "Zoom"
                color: root.secondaryText
                font.pixelSize: 12
            }

            Slider {
                id: timelineZoomSlider
                from: 24
                to: 240
                value: appController.timelinePixelsPerSecond
                Layout.preferredWidth: 180
                onMoved: appController.set_timeline_zoom(value)
            }

            Label {
                text: Math.round(appController.timelinePixelsPerSecond) + " px/s"
                color: root.textMuted
                font.pixelSize: 12
                Layout.preferredWidth: 64
            }

            Slider {
                id: timelineScrollSlider
                from: 0
                to: Math.max(0, appController.timelineDurationSeconds - appController.timelineVisibleSeconds)
                value: appController.timelineScrollSeconds
                Layout.fillWidth: true
                onMoved: appController.set_timeline_scroll_seconds(value)
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            ListView {
                id: timelineRows
                Layout.fillWidth: true
                Layout.fillHeight: true
                model: appController.trackModel
                clip: true
                onWidthChanged: root.updateTimelineVisibleSeconds()

                delegate: Row {
                    width: timelineRows.width
                    height: root.timelineRowHeight
                    spacing: 0

                    Rectangle {
                        width: root.timelineLabelWidth
                        height: parent.height
                        color: index % 2 === 0 ? root.panelBackground : root.laneBackground
                        border.color: appController.selectedTrackId === trackId ? root.focusAccent : root.borderSubtle

                        Column {
                            anchors.fill: parent
                            anchors.margins: 10
                            spacing: 4

                            Text {
                                text: name
                                color: root.textPrimary
                                font.pixelSize: 14
                                elide: Text.ElideRight
                                width: parent.width
                            }

                            Text {
                                text: trackType + " - " + resultState + " - " + markerCount + " markers"
                                color: resultState === "failed" || resultState === "stale" ? root.statusErrorColor : root.textMuted
                                font.pixelSize: 12
                                elide: Text.ElideRight
                                width: parent.width
                            }

                            Text {
                                text: cacheRefCount > 0 ? artifactKinds + " artifact" : ""
                                color: root.artifactAccent
                                font.pixelSize: 12
                                elide: Text.ElideRight
                                width: parent.width
                                visible: cacheRefCount > 0
                            }

                            Text {
                                text: error
                                visible: error.length > 0
                                color: "#fca5a5"
                                font.pixelSize: 11
                                elide: Text.ElideRight
                                width: parent.width
                            }

                            ProgressBar {
                                width: parent.width
                                from: 0
                                to: 1
                                value: jobProgress
                                visible: activeJobId.length > 0
                            }
                        }

                        MouseArea {
                            anchors.fill: parent
                            acceptedButtons: Qt.LeftButton
                            onClicked: appController.select_track(trackId)
                        }
                    }

                    Rectangle {
                        width: Math.max(0, parent.width - root.timelineLabelWidth)
                        height: parent.height
                        color: index % 2 === 0 ? root.laneBackground : root.laneBackgroundAlt
                        border.color: appController.selectedTrackId === trackId ? root.focusAccent : root.borderSubtle
                        clip: true

                        Rectangle {
                            id: waveformCenterLine
                            x: root.timelineLeftPadding
                            y: Math.round(parent.height / 2)
                            width: Math.max(0, parent.width - root.timelineLeftPadding)
                            height: 1
                            color: root.borderSubtle
                            visible: waveformSamples.length > 0
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

                        Repeater {
                            model: markerSpans
                            Rectangle {
                                width: Math.max(8, (modelData.duration > 0 ? modelData.duration : 0.08) * appController.timelinePixelsPerSecond)
                                height: parent.height - 18
                                x: root.timelineX(modelData.timestamp)
                                y: 9
                                visible: x + width >= root.timelineLeftPadding && x <= parent.width
                                radius: 2
                                color: modelData.color

                                Text {
                                    anchors.centerIn: parent
                                    width: parent.width - 6
                                    text: modelData.label
                                    color: root.markerLabelText
                                    font.pixelSize: 10
                                    font.bold: true
                                    horizontalAlignment: Text.AlignHCenter
                                    elide: Text.ElideRight
                                    visible: parent.width >= 36 && modelData.label.length > 0
                                }
                            }
                        }

                        Rectangle {
                            id: playhead
                            width: 2
                            height: parent.height
                            x: root.timelineX(appController.playback.positionSeconds)
                            color: root.focusAccent
                            visible: appController.playback.sourcePath.length > 0
                                && x >= root.timelineLeftPadding
                                && x <= parent.width
                            z: 10
                        }

                        MouseArea {
                            anchors.fill: parent
                            acceptedButtons: Qt.LeftButton
                            onClicked: function(mouse) {
                                appController.select_track(trackId)
                                root.seekTimelineAtX(mouse.x)
                            }
                        }
                    }
                }
            }

            Rectangle {
                id: inspectorPanel
                Layout.preferredWidth: 260
                Layout.fillHeight: true
                color: root.panelBackground
                border.color: root.borderSubtle
                property string selectedMarkerId: ""

                Connections {
                    target: appController
                    function onSelectedTrackIdChanged() {
                        inspectorPanel.selectedMarkerId = ""
                    }
                    function onSelectedMarkerIdsChanged() {
                        root.syncMarkerEditorFromSelection()
                    }
                }

                Column {
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 8

                    Label {
                        text: "Inspector"
                        color: root.textPrimary
                        font.bold: true
                    }

                    Text {
                        text: appController.selectedTrackId.length === 0 ? "No track selected" : ""
                        visible: appController.selectedTrackId.length === 0
                        color: root.textMuted
                        font.pixelSize: 12
                        wrapMode: Text.WordWrap
                        width: parent.width
                    }

                    TextField {
                        id: markerTimestampField
                        placeholderText: "Timestamp"
                        text: "0.0"
                        enabled: appController.selectedTrackIsEditable
                        width: parent.width
                    }

                    TextField {
                        id: markerLabelField
                        placeholderText: "Label"
                        text: "Cue"
                        enabled: appController.selectedTrackIsEditable
                        width: parent.width
                    }

                    TextField {
                        id: markerCategoryField
                        placeholderText: "Category"
                        text: "cue"
                        enabled: appController.selectedTrackIsEditable
                        width: parent.width
                    }

                    ComboBox {
                        id: markerColorPicker
                        model: root.markerColorOptions
                        textRole: "label"
                        valueRole: "key"
                        enabled: appController.selectedTrackIsEditable
                        width: parent.width
                        delegate: ItemDelegate {
                            width: markerColorPicker.width
                            text: modelData.label
                            contentItem: Row {
                                spacing: 8
                                Rectangle {
                                    width: 12
                                    height: 12
                                    radius: 6
                                    color: modelData.color
                                    anchors.verticalCenter: parent.verticalCenter
                                }
                                Text {
                                    text: modelData.label
                                    color: root.textPrimary
                                    anchors.verticalCenter: parent.verticalCenter
                                }
                            }
                        }
                    }

                    ScrollView {
                        id: markerScroll
                        width: parent.width
                        height: 120
                        clip: true

                        Column {
                            id: markerList
                            width: markerScroll.availableWidth
                            spacing: 2

                            Repeater {
                                model: appController.selectedTrackMarkers
                                delegate: Rectangle {
                                    required property var modelData
                                    width: markerList.width
                                    height: 34
                                    radius: 3
                                    color: modelData.selected ? root.selectedMarkerBackground : "transparent"
                                    border.color: modelData.selected ? modelData.color : "transparent"

                                    Rectangle {
                                        id: markerColorSwatch
                                        width: 10
                                        height: 10
                                        radius: 5
                                        color: modelData.color
                                        anchors.left: parent.left
                                        anchors.leftMargin: 4
                                        anchors.verticalCenter: parent.verticalCenter
                                    }

                                    Text {
                                        anchors.left: markerColorSwatch.right
                                        anchors.leftMargin: 8
                                        anchors.right: parent.right
                                        anchors.rightMargin: 4
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: Number(modelData.timestamp).toFixed(2) + "  " + modelData.label
                                        color: root.textPrimary
                                        elide: Text.ElideRight
                                    }

                                    MouseArea {
                                        anchors.fill: parent
                                        onClicked: function(mouse) {
                                            appController.toggle_marker_selection(modelData.id, (mouse.modifiers & Qt.ShiftModifier) !== 0)
                                            root.syncMarkerEditorFromSelection()
                                        }
                                    }
                                }
                            }
                        }
                    }

                    Button {
                        text: "Add Cue"
                        enabled: appController.selectedTrackId.length > 0 && appController.selectedTrackIsEditable
                        onClicked: appController.add_marker_to_selected_track(
                            Number(markerTimestampField.text),
                            markerLabelField.text,
                            markerCategoryField.text,
                            markerColorPicker.currentValue
                        )
                    }

                    Button {
                        text: "Delete Cue"
                        enabled: inspectorPanel.selectedMarkerId.length > 0 && appController.selectedTrackIsEditable
                        onClicked: {
                            if (appController.delete_marker_from_selected_track(inspectorPanel.selectedMarkerId)) {
                                inspectorPanel.selectedMarkerId = ""
                            }
                        }
                    }

                    Button {
                        text: "Update Cue"
                        enabled: appController.selectedTrackIsEditable && root.selectedMarkerCount() === 1
                        onClicked: appController.update_selected_marker(
                            Number(markerTimestampField.text),
                            markerLabelField.text,
                            markerCategoryField.text,
                            markerColorPicker.currentValue
                        )
                    }

                    Button {
                        text: root.selectedMarkerCount() > 0 ? "Apply To Selected" : "Apply To Track"
                        enabled: appController.selectedTrackIsEditable && appController.selectedTrackMarkers.length > 0
                        onClicked: appController.bulk_update_selected_markers(
                            markerLabelField.text,
                            markerCategoryField.text,
                            markerColorPicker.currentValue
                        )
                    }
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 34
            color: root.footerBackground
            border.color: root.borderSubtle

            Text {
                anchors.verticalCenter: parent.verticalCenter
                anchors.left: parent.left
                anchors.leftMargin: 12
                width: parent.width - 24
                text: root.statusError.length > 0
                    ? root.statusError
                    : (appController.projectPath.length > 0 ? appController.projectPath : "Unsaved project")
                color: root.statusError.length > 0 ? root.statusErrorColor : root.textMuted
                elide: Text.ElideMiddle
                font.pixelSize: 12
            }
        }
    }
}
