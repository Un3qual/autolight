import QtQuick

Item {
    id: root
    property string geometryJson: ""

    function parseBands() {
        if (!root.geometryJson || root.geometryJson.trim().length === 0) return []
        try {
            var parsed = JSON.parse(root.geometryJson)
            return Array.isArray(parsed.bands) ? parsed.bands : []
        } catch (error) {
            return []
        }
    }

    onGeometryJsonChanged: analysisCanvas.requestPaint()
    onWidthChanged: analysisCanvas.requestPaint()
    onHeightChanged: analysisCanvas.requestPaint()

    Canvas {
        id: analysisCanvas
        anchors.fill: parent
        renderTarget: Canvas.FramebufferObject
        onPaint: {
            var context = getContext("2d")
            context.clearRect(0, 0, width, height)
            var bands = root.parseBands()
            for (var bandIndex = 0; bandIndex < bands.length; ++bandIndex) {
                var band = bands[bandIndex]
                var rects = Array.isArray(band.rects) ? band.rects : []
                context.fillStyle = band.color || "#93c5fd"
                for (var rectIndex = 0; rectIndex < rects.length; ++rectIndex) {
                    var rect = rects[rectIndex]
                    var rectX = Number(rect.x)
                    var rectY = Number(rect.y)
                    var rectWidth = Number(rect.width)
                    var rectHeight = Number(rect.height)
                    if (!isFinite(rectX) || !isFinite(rectY) || !isFinite(rectWidth) || !isFinite(rectHeight)) continue
                    if (rectWidth <= 0 || rectHeight <= 0) continue
                    context.fillRect(rectX, rectY, rectWidth, rectHeight)
                }
            }
        }
    }
}
