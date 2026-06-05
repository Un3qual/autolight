import QtQuick

Item {
    id: root
    property var appController
    property real laneWidth: width
    property real contentLeftPadding: 0
    property bool allowScrub: false
    property real lastPinchScale: 1
    signal scrubRequested(real x, real laneWidth)

    function finiteNumber(value) {
        var number = Number(value)
        return isFinite(number) ? number : 0
    }

    function dominantHorizontal(pixelX, pixelY, angleX, angleY) {
        var horizontal = Math.abs(pixelX) > 0 ? Math.abs(pixelX) : Math.abs(angleX)
        var vertical = Math.abs(pixelY) > 0 ? Math.abs(pixelY) : Math.abs(angleY)
        return horizontal > 0 && horizontal >= vertical
    }

    function zoomFactorFromWheel(delta) {
        var value = root.finiteNumber(delta)
        return value === 0 ? 1 : Math.pow(1.0015, value)
    }

    function contentX(x) {
        return Math.max(0, root.finiteNumber(x) - root.finiteNumber(root.contentLeftPadding))
    }

    function contentLaneWidth() {
        return Math.max(0, root.finiteNumber(root.laneWidth) - root.finiteNumber(root.contentLeftPadding))
    }

    function beginNavigation() {
        if (root.appController) root.appController.begin_timeline_user_navigation()
    }

    function endNavigation() {
        if (root.appController) root.appController.end_timeline_user_navigation()
    }

    function extendWheelNavigationQuietPeriod() {
        root.beginNavigation()
        wheelNavigationQuietTimer.restart()
    }

    function scrubAt(x) {
        if (!root.appController) return
        root.scrubRequested(root.contentX(x), root.contentLaneWidth())
    }

    WheelHandler {
        target: null
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad

        onWheel: function(event) {
            if (!root.appController) {
                event.accepted = false
                return
            }
            var pixelX = root.finiteNumber(event.pixelDelta.x)
            var pixelY = root.finiteNumber(event.pixelDelta.y)
            var angleX = root.finiteNumber(event.angleDelta.x) / 8
            var angleY = root.finiteNumber(event.angleDelta.y) / 8
            var zoomModifier = (event.modifiers & Qt.ControlModifier) !== 0
                || (event.modifiers & Qt.MetaModifier) !== 0
            if (zoomModifier) {
                var zoomDelta = pixelY !== 0 ? pixelY : angleY
                var factor = root.zoomFactorFromWheel(zoomDelta)
                if (factor !== 1) {
                    root.extendWheelNavigationQuietPeriod()
                    root.appController.zoom_timeline_by_factor(
                        factor,
                        root.contentX(event.position.x),
                        root.contentLaneWidth()
                    )
                    event.accepted = true
                    return
                }
                event.accepted = false
                return
            }

            var horizontalPixels = 0
            if (root.dominantHorizontal(pixelX, pixelY, angleX, angleY)) {
                horizontalPixels = pixelX !== 0 ? -pixelX : -angleX
            } else if ((event.modifiers & Qt.ShiftModifier) !== 0) {
                horizontalPixels = pixelY !== 0 ? -pixelY : -angleY
            }
            if (horizontalPixels !== 0) {
                root.extendWheelNavigationQuietPeriod()
                root.appController.scroll_timeline_by_pixels(horizontalPixels)
                event.accepted = true
                return
            }
            event.accepted = false
        }
    }

    Timer {
        id: wheelNavigationQuietTimer
        interval: 220
        repeat: false
        onTriggered: root.endNavigation()
    }

    PinchHandler {
        target: null
        acceptedDevices: PointerDevice.TouchPad | PointerDevice.TouchScreen

        onActiveChanged: {
            if (active) {
                root.lastPinchScale = 1
                root.beginNavigation()
            } else {
                root.lastPinchScale = 1
                root.endNavigation()
            }
        }
        onScaleChanged: {
            if (!root.appController) return
            var scaleValue = Number(scale)
            if (!isFinite(scaleValue) || scaleValue <= 0) return
            var factor = scaleValue / Math.max(root.lastPinchScale, 0.0001)
            root.lastPinchScale = scaleValue
            root.appController.zoom_timeline_by_factor(
                factor,
                root.contentX(centroid.position.x),
                root.contentLaneWidth()
            )
        }
    }

    MouseArea {
        anchors.fill: parent
        enabled: root.allowScrub
        acceptedButtons: Qt.LeftButton
        hoverEnabled: false
        onPressed: function(mouse) {
            root.beginNavigation()
            root.scrubAt(mouse.x)
        }
        onPositionChanged: function(mouse) {
            if (pressed) root.scrubAt(mouse.x)
        }
        onReleased: function(mouse) {
            root.scrubAt(mouse.x)
            root.endNavigation()
        }
        onCanceled: root.endNavigation()
    }
}
