import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ToolBar {
    id: root
    property var appController
    property int compactButtonHeight: 30
    property color toolbarForeground: "#111318"
    signal newRequested()
    signal openRequested()
    signal saveAsRequested()
    signal demoRequested()
    signal importAudioRequested()

    RowLayout {
        anchors.fill: parent
        spacing: 8

        Label {
            text: root.appController.projectName
            color: root.toolbarForeground
            font.pixelSize: 16
            font.bold: true
            Layout.leftMargin: 12
        }

        Item { Layout.fillWidth: true }

        RowLayout {
            id: fileActions
            spacing: 6

            Button { text: "New"; implicitHeight: root.compactButtonHeight; onClicked: root.newRequested() }
            Button { text: "Open"; implicitHeight: root.compactButtonHeight; onClicked: root.openRequested() }
            Button {
                text: "Save"
                implicitHeight: root.compactButtonHeight
                onClicked: root.appController.projectPath.length > 0 ? root.appController.save_project("") : root.saveAsRequested()
            }
            Button { text: "Save As"; implicitHeight: root.compactButtonHeight; onClicked: root.saveAsRequested() }
            Button { text: "Demo"; implicitHeight: root.compactButtonHeight; onClicked: root.demoRequested() }
            Button { text: "Import Audio"; implicitHeight: root.compactButtonHeight; onClicked: root.importAudioRequested() }
        }
    }
}
