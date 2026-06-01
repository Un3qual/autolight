import QtQuick

Rectangle {
    id: root
    property var marker: ({ duration: 0, timestamp: 0, label: "", color: "transparent" })
    property var appController
    property string markerId: root.marker.id || ""
    property real timestamp: Number(root.marker.timestamp || 0)
    property real duration: Number(root.marker.duration || 0)
    property bool editable: false
    property real pixelsPerSecond: 96
    property color markerColor: root.marker.color || "#22d3ee"
    property string markerLabel: root.marker.label || ""
    property real timelineLeftPadding: 24
    property color markerLabelText: "#111318"
    signal selected(string markerId, bool additive)

    function timelineX(seconds) {
        return root.timelineLeftPadding + (seconds - root.appController.timelineScrollSeconds) * root.appController.timelinePixelsPerSecond
    }

    width: Math.max(8, (root.marker.duration > 0 ? root.marker.duration : 0.08) * root.appController.timelinePixelsPerSecond)
    height: parent.height - 18
    x: root.timelineX(root.marker.timestamp)
    y: 9
    visible: x + width >= root.timelineLeftPadding && x <= parent.width
    radius: 2
    color: root.markerColor

    MouseArea {
        id: bodyDrag
        anchors.fill: parent
        enabled: root.editable
        drag.target: null
        property real pressX: 0
        property real lastPreviewDelta: 0

        onPressed: function(mouse) {
            pressX = mouse.x
            root.selected(root.markerId, (mouse.modifiers & Qt.ShiftModifier) !== 0)
        }

        onPositionChanged: function(mouse) {
            lastPreviewDelta = (mouse.x - pressX) / Math.max(1, root.pixelsPerSecond)
        }

        onReleased: function(mouse) {
            var bypass = (mouse.modifiers & Qt.AltModifier) !== 0
            var delta = (mouse.x - pressX) / Math.max(1, root.pixelsPerSecond)
            root.appController.move_selected_markers(delta, bypass)
            lastPreviewDelta = 0
        }
    }

    Rectangle {
        id: rightResizeHandle
        width: 8
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        anchors.right: parent.right
        color: "transparent"
        visible: root.editable

        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.SizeHorCursor
            property real startWidth: 0
            property real startX: 0
            onPressed: function(mouse) {
                startWidth = root.width
                startX = mouse.x
            }
            onReleased: function(mouse) {
                var widthDelta = mouse.x - startX
                var nextDuration = Math.max(0, (startWidth + widthDelta) / Math.max(1, root.pixelsPerSecond))
                root.appController.resize_marker(root.markerId, nextDuration)
            }
        }
    }

    Text {
        anchors.centerIn: parent
        width: parent.width - 6
        text: root.markerLabel
        color: root.markerLabelText
        font.pixelSize: 10
        font.bold: true
        horizontalAlignment: Text.AlignHCenter
        elide: Text.ElideRight
        visible: parent.width >= 36 && root.markerLabel.length > 0
    }
}
