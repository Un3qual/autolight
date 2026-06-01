import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: root
    property var appController
    property string statusError: ""
    property color footerBackground: "#111318"
    property color borderSubtle: "#2f333d"
    property color statusErrorColor: "#f87171"
    property color textMuted: "#a1a1aa"

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
            : (root.appController.projectPath.length > 0 ? root.appController.projectPath : "Unsaved project")
        color: root.statusError.length > 0 ? root.statusErrorColor : root.textMuted
        elide: Text.ElideMiddle
        font.pixelSize: 12
    }
}
