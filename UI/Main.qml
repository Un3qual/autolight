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
    readonly property real timelinePixelsPerSecond: 96
    readonly property real timelineLeftPadding: 24
    readonly property real timelineRulerHeight: 32
    readonly property real defaultMarkerDuration: 8.0
    readonly property real defaultMarkerInterval: 0.5

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

                Button {
                    text: "Run"
                    enabled: appController.selectedTrackId.length > 0
                    onClicked: appController.run_track(appController.selectedTrackId)
                }

                Button {
                    text: "Cancel"
                    enabled: appController.selectedTrackId.length > 0
                    onClicked: appController.cancel_selected_job()
                }

                Button {
                    text: "Rerun"
                    enabled: appController.selectedTrackCanRerun
                    onClicked: appController.rerun_track(appController.selectedTrackId)
                }

                Button {
                    text: "Check Cache"
                    onClicked: appController.refresh_cache_status()
                }

                Button {
                    text: "Derive Editable"
                    enabled: appController.selectedTrackId.length > 0
                    onClicked: appController.create_editable_track_from_track(appController.selectedTrackId)
                }

                Button {
                    text: "Load Demo"
                    onClicked: root.demoProjectWithConfirmation()
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
                Layout.preferredWidth: 280
                Layout.fillHeight: true
                color: "#1c1f26"
                border.color: "#2f333d"
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                color: "#1c1f26"

                Row {
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.left: parent.left
                    anchors.leftMargin: root.timelineLeftPadding
                    spacing: root.timelinePixelsPerSecond

                    Repeater {
                        model: 9
                        Text {
                            text: index + "s"
                            color: "#a1a1aa"
                            font.pixelSize: 12
                        }
                    }
                }
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

                delegate: Row {
                    width: timelineRows.width
                    height: 74
                    spacing: 0

                    Rectangle {
                        width: 280
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
                        width: Math.max(0, parent.width - 280)
                        height: parent.height
                        color: index % 2 === 0 ? "#171a20" : "#14171d"
                        border.color: appController.selectedTrackId === trackId ? "#facc15" : "#2f333d"

                        Repeater {
                            model: markerSpans
                            Rectangle {
                                width: Math.max(8, (modelData.duration > 0 ? modelData.duration : 0.08) * root.timelinePixelsPerSecond)
                                height: parent.height - 18
                                x: Math.max(0, Math.min(parent.width - width, root.timelineLeftPadding + modelData.timestamp * root.timelinePixelsPerSecond))
                                y: 9
                                radius: 2
                                color: trackType === "editable" ? "#67e8f9" : "#a7f3d0"
                            }
                        }

                        MouseArea {
                            anchors.fill: parent
                            acceptedButtons: Qt.LeftButton
                            onClicked: appController.select_track(trackId)
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
                    }

                    TextField {
                        id: markerLabelField
                        placeholderText: "Label"
                        text: "Cue"
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
                                    height: 28
                                    color: inspectorPanel.selectedMarkerId === modelData.id ? "#2f4366" : "transparent"

                                    Text {
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: Number(modelData.timestamp).toFixed(2) + "  " + modelData.label
                                        color: "#f4f4f5"
                                        elide: Text.ElideRight
                                        width: parent.width
                                    }

                                    MouseArea {
                                        anchors.fill: parent
                                        onClicked: inspectorPanel.selectedMarkerId = modelData.id
                                    }
                                }
                            }
                        }
                    }

                    Button {
                        text: "Add Cue"
                        enabled: appController.selectedTrackId.length > 0
                        onClicked: appController.add_marker_to_selected_track(
                            Number(markerTimestampField.text),
                            markerLabelField.text
                        )
                    }

                    Button {
                        text: "Delete Cue"
                        enabled: inspectorPanel.selectedMarkerId.length > 0
                        onClicked: {
                            if (appController.delete_marker_from_selected_track(inspectorPanel.selectedMarkerId)) {
                                inspectorPanel.selectedMarkerId = ""
                            }
                        }
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
                text: appController.lastError.length > 0
                    ? appController.lastError
                    : (appController.projectPath.length > 0 ? appController.projectPath : "Unsaved project")
                color: appController.lastError.length > 0 ? "#f87171" : "#a1a1aa"
                elide: Text.ElideMiddle
                font.pixelSize: 12
            }
        }
    }
}
