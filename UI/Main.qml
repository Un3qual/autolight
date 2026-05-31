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

    FileDialog {
        id: openProjectDialog
        title: "Open Autolight Project"
        nameFilters: ["Autolight projects (*.autolight)"]
        fileMode: FileDialog.OpenFile
        onAccepted: appController.open_project(String(selectedFile))
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

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        ToolBar {
            Layout.fillWidth: true

            RowLayout {
                anchors.fill: parent
                spacing: 12

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
                    onClicked: appController.new_project()
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
                    onClicked: appController.add_fixed_interval_track(appController.selectedTrackId, 8.0, 0.5)
                }

                Button {
                    text: "Run"
                    enabled: appController.selectedTrackId.length > 0
                    onClicked: appController.run_track(appController.selectedTrackId)
                }

                Button {
                    text: "Derive Editable"
                    enabled: appController.selectedTrackId.length > 0
                    onClicked: appController.create_editable_track_from_track(appController.selectedTrackId)
                }

                Button {
                    text: "Load Demo"
                    onClicked: appController.load_demo_project()
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
                            color: resultState === "failed" ? "#f87171" : "#a1a1aa"
                            font.pixelSize: 12
                            elide: Text.ElideRight
                            width: parent.width
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
