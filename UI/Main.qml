import QtQuick
import QtQuick.Controls
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
                    text: "Load Demo"
                    onClicked: appController.load_demo_project()
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.preferredHeight: 42
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
                    border.color: "#343842"

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
                }

                Rectangle {
                    width: Math.max(0, parent.width - 280)
                    height: parent.height
                    color: index % 2 === 0 ? "#171a20" : "#14171d"
                    border.color: "#2f333d"

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
                }
            }
        }
    }
}
