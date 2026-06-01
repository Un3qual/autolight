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
    readonly property real defaultMarkerDuration: 8.0
    readonly property real defaultMarkerInterval: 0.5
    readonly property color controlTextColor: "#f4f4f5"
    readonly property color controlMutedTextColor: "#a1a1aa"
    readonly property string statusError: appController.lastError.length > 0 ? appController.lastError : appController.playback.lastError
    readonly property var markerColorOptions: [
        { key: "cyan", label: "Cyan", color: "#67e8f9" },
        { key: "green", label: "Green", color: "#a7f3d0" },
        { key: "amber", label: "Amber", color: "#fbbf24" },
        { key: "violet", label: "Violet", color: "#c4b5fd" },
        { key: "rose", label: "Rose", color: "#fda4af" },
        { key: "blue", label: "Blue", color: "#93c5fd" }
    ]

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
            color: "#f4f4f5"
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
                    color: "#f4f4f5"
                    font.pixelSize: 16
                    font.bold: true
                    Layout.leftMargin: 12
                }

                Item { Layout.fillWidth: true }

                Button {
                    text: "New"
                    onClicked: root.newProjectWithConfirmation()
                }

                Button {
                    text: "Open"
                    onClicked: openProjectDialog.open()
                }

                Button {
                    text: "Save"
                    onClicked: appController.projectPath.length > 0 ? appController.save_project("") : saveProjectDialog.open()
                }

                Button {
                    text: "Save As"
                    onClicked: saveProjectDialog.open()
                }

                Button {
                    text: "Import Audio"
                    onClicked: importAudioDialog.open()
                }

                Button {
                    text: "Add Markers"
                    enabled: appController.selectedTrackId.length > 0
                    onClicked: appController.add_fixed_interval_track(appController.selectedTrackId, root.defaultMarkerDuration, root.defaultMarkerInterval)
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
                color: "#1c1f26"
                border.color: "#2f333d"
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                color: "#1c1f26"

                Repeater {
                    model: Math.ceil(appController.timelineVisibleSeconds) + 1
                    Text {
                        property real tickSecond: Math.ceil(appController.timelineScrollSeconds) + index
                        x: root.timelineX(tickSecond)
                        y: 9
                        text: tickSecond + "s"
                        color: "#a1a1aa"
                        font.pixelSize: 12
                    }
                }
            }
        }

        RowLayout {
            id: trackActionControls
            Layout.fillWidth: true
            Layout.leftMargin: 12
            Layout.rightMargin: 12
            spacing: 6

            ComboBox {
                id: transformPicker
                model: appController.transformModel
                textRole: "name"
                valueRole: "transformId"
                Layout.preferredWidth: 150
                palette.text: root.controlTextColor
                palette.buttonText: root.controlTextColor
            }

            TextField {
                id: transformParamsField
                text: "{\"duration\": 8.0, \"interval\": 0.5}"
                placeholderText: "JSON params"
                Layout.preferredWidth: 150
                color: root.controlTextColor
                placeholderTextColor: root.controlMutedTextColor
                selectedTextColor: root.controlTextColor
                selectionColor: "#2563eb"
            }

            Button {
                text: "Transform"
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
                text: "Vocals"
                palette.buttonText: root.controlTextColor
                enabled: appController.selectedTrackId.length > 0
                onClicked: appController.add_vocals_stem_track(appController.selectedTrackId)
            }

            Button {
                text: "Run"
                palette.buttonText: root.controlTextColor
                enabled: appController.selectedTrackCanRerun && !appController.selectedTrackHasRunningJob
                onClicked: appController.run_track(appController.selectedTrackId)
            }

            Button {
                text: "Cancel"
                palette.buttonText: root.controlTextColor
                enabled: appController.selectedTrackHasRunningJob
                onClicked: appController.cancel_selected_job()
            }

            Button {
                text: "Rerun"
                palette.buttonText: root.controlTextColor
                enabled: appController.selectedTrackCanRerun && !appController.selectedTrackHasRunningJob
                onClicked: appController.rerun_track(appController.selectedTrackId)
            }

            Button {
                text: "Cache"
                palette.buttonText: root.controlTextColor
                onClicked: appController.refresh_cache_status()
            }

            Button {
                text: "Editable"
                palette.buttonText: root.controlTextColor
                enabled: appController.selectedTrackId.length > 0
                onClicked: appController.create_editable_track_from_track(appController.selectedTrackId)
            }

            Button {
                text: "Demo"
                palette.buttonText: root.controlTextColor
                onClicked: root.demoProjectWithConfirmation()
            }
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
                    color: "#d4d4d8"
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
            Layout.fillWidth: true
            Layout.leftMargin: 12
            Layout.rightMargin: 12
            spacing: 10

            Label {
                text: "Zoom"
                color: "#d4d4d8"
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
                color: "#a1a1aa"
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
                    height: 74
                    spacing: 0

                    Rectangle {
                        width: root.timelineLabelWidth
                        height: parent.height
                        color: index % 2 === 0 ? "#23262d" : "#1f2229"
                        border.color: appController.selectedTrackId === trackId ? "#facc15" : "#343842"

                        Column {
                            anchors.fill: parent
                            anchors.margins: 10
                            spacing: 4

                            Text {
                                text: name
                                color: "#f4f4f5"
                                font.pixelSize: 14
                                elide: Text.ElideRight
                                width: parent.width
                            }

                            Text {
                                text: trackType + " - " + resultState + " - " + markerCount + " markers"
                                color: resultState === "failed" || resultState === "stale" ? "#f87171" : "#a1a1aa"
                                font.pixelSize: 12
                                elide: Text.ElideRight
                                width: parent.width
                            }

                            Text {
                                text: cacheRefCount > 0 ? artifactKinds + " artifact" : ""
                                color: "#93c5fd"
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
                        color: index % 2 === 0 ? "#171a20" : "#14171d"
                        border.color: appController.selectedTrackId === trackId ? "#facc15" : "#2f333d"
                        clip: true

                        Rectangle {
                            id: waveformCenterLine
                            x: root.timelineLeftPadding
                            y: Math.round(parent.height / 2)
                            width: Math.max(0, parent.width - root.timelineLeftPadding)
                            height: 1
                            color: "#2f333d"
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
                                    color: "#111318"
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
                            color: "#facc15"
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
                color: "#1c1f26"
                border.color: "#2f333d"
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
                        color: "#f4f4f5"
                        font.bold: true
                    }

                    TextField {
                        id: markerTimestampField
                        placeholderText: "Timestamp"
                        text: "0.0"
                        width: parent.width
                    }

                    TextField {
                        id: markerLabelField
                        placeholderText: "Label"
                        text: "Cue"
                        width: parent.width
                    }

                    TextField {
                        id: markerCategoryField
                        placeholderText: "Category"
                        text: "cue"
                        width: parent.width
                    }

                    ComboBox {
                        id: markerColorPicker
                        model: root.markerColorOptions
                        textRole: "label"
                        valueRole: "key"
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
                                    color: "#f4f4f5"
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
                                    color: modelData.selected ? "#2f4366" : "transparent"
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
                                        color: "#f4f4f5"
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
            color: "#111318"
            border.color: "#2f333d"

            Text {
                anchors.verticalCenter: parent.verticalCenter
                anchors.left: parent.left
                anchors.leftMargin: 12
                width: parent.width - 24
                text: root.statusError.length > 0
                    ? root.statusError
                    : (appController.projectPath.length > 0 ? appController.projectPath : "Unsaved project")
                color: root.statusError.length > 0 ? "#f87171" : "#a1a1aa"
                elide: Text.ElideMiddle
                font.pixelSize: 12
            }
        }
    }
}
