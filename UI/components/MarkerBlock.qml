import QtQuick

Rectangle {
    id: root
    property var marker: ({ duration: 0, timestamp: 0, label: "", color: "transparent" })
    property var appController
    property real timelineLeftPadding: 24
    property color markerLabelText: "#111318"

    function timelineX(seconds) {
        return root.timelineLeftPadding + (seconds - root.appController.timelineScrollSeconds) * root.appController.timelinePixelsPerSecond
    }

    width: Math.max(8, (root.marker.duration > 0 ? root.marker.duration : 0.08) * root.appController.timelinePixelsPerSecond)
    height: parent.height - 18
    x: root.timelineX(root.marker.timestamp)
    y: 9
    visible: x + width >= root.timelineLeftPadding && x <= parent.width
    radius: 2
    color: root.marker.color

    Text {
        anchors.centerIn: parent
        width: parent.width - 6
        text: root.marker.label
        color: root.markerLabelText
        font.pixelSize: 10
        font.bold: true
        horizontalAlignment: Text.AlignHCenter
        elide: Text.ElideRight
        visible: parent.width >= 36 && root.marker.label.length > 0
    }
}
