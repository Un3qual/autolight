import QtQuick

Rectangle {
    id: root
    property var marker: ({ duration: 0, timestamp: 0, label: "", color: "transparent" })
    property var appController
    property string trackId: ""
    property string markerId: root.marker.id || ""
    property real timestamp: Number(root.marker.timestamp || 0)
    property real duration: Number(root.marker.duration || 0)
    property real baseX: root.timelineX(root.marker.timestamp)
    property real lastPreviewDelta: 0
    property bool markerSelected: false
    property bool editable: false
    property real pixelsPerSecond: 96
    property real dragThresholdPixels: 2
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
    x: root.baseX + root.lastPreviewDelta * root.pixelsPerSecond
    y: 9
    visible: x + width >= root.timelineLeftPadding && x <= parent.width
    radius: 2
    color: root.markerColor

    MouseArea {
        id: bodyDrag
        anchors.fill: parent
        enabled: root.editable
        drag.target: null
        property real pressParentX: 0

        function parentX(mouse) {
            return mapToItem(root.parent, mouse.x, mouse.y).x
        }

        onPressed: function(mouse) {
            pressParentX = parentX(mouse)
            root.lastPreviewDelta = 0
            var additive = (mouse.modifiers & Qt.ShiftModifier) !== 0
            root.appController.select_track(root.trackId)
            if (!root.markerSelected) {
                root.selected(root.markerId, additive)
            }
        }

        onPositionChanged: function(mouse) {
            var pixelDelta = parentX(mouse) - pressParentX
            root.lastPreviewDelta = pixelDelta / Math.max(1, root.pixelsPerSecond)
        }

        onReleased: function(mouse) {
            var pixelDelta = parentX(mouse) - pressParentX
            if (Math.abs(pixelDelta) < root.dragThresholdPixels) {
                root.lastPreviewDelta = 0
                return
            }
            var bypass = (mouse.modifiers & Qt.AltModifier) !== 0
            var delta = pixelDelta / Math.max(1, root.pixelsPerSecond)
            root.appController.select_track(root.trackId)
            root.appController.move_selected_markers(delta, bypass)
            root.lastPreviewDelta = 0
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
            property real startParentX: 0
            property real startDuration: 0

            function parentX(mouse) {
                return mapToItem(root.parent, mouse.x, mouse.y).x
            }

            onPressed: function(mouse) {
                startParentX = parentX(mouse)
                startDuration = root.duration
                root.appController.select_track(root.trackId)
            }
            onReleased: function(mouse) {
                var widthDelta = parentX(mouse) - startParentX
                if (Math.abs(widthDelta) < root.dragThresholdPixels) {
                    return
                }
                var nextDuration = Math.max(0, startDuration + widthDelta / Math.max(1, root.pixelsPerSecond))
                root.appController.select_track(root.trackId)
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
