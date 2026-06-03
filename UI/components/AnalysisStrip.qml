import QtQuick

Canvas {
    id: root
    required property var samples
    required property string stripKind
    property real durationSeconds: 0
    property real scrollSeconds: 0
    property real pixelsPerSecond: 96
    property real leftPadding: 24
    property color energyColor: "#facc15"
    width: parent ? parent.width : 0
    height: 16
    visible: root.sampleCount() > 0

    function finiteNumber(value, fallbackValue) {
        var numericValue = Number(value)
        return isFinite(numericValue) ? numericValue : fallbackValue
    }

    function clampedUnit(value) {
        return Math.max(0, Math.min(1, root.finiteNumber(value, 0)))
    }

    function sampleCount() {
        return root.samples && root.samples.length ? root.samples.length : 0
    }

    onSamplesChanged: requestPaint()
    onStripKindChanged: requestPaint()
    onScrollSecondsChanged: requestPaint()
    onPixelsPerSecondChanged: requestPaint()
    onLeftPaddingChanged: requestPaint()
    onEnergyColorChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()

    onPaint: {
        var ctx = getContext("2d")
        ctx.clearRect(0, 0, width, height)
        var count = root.sampleCount()
        if (count === 0 || width <= 0 || height <= 0) {
            return
        }
        var safeScrollSeconds = root.finiteNumber(root.scrollSeconds, 0)
        var safePixelsPerSecond = Math.max(0, root.finiteNumber(root.pixelsPerSecond, 96))
        var safeLeftPadding = root.finiteNumber(root.leftPadding, 24)
        var drawableSamples = []
        for (var index = 0; index < count; index += 1) {
            var sample = root.samples[index] || {}
            var sampleTime = root.finiteNumber(sample.time, NaN)
            if (!isFinite(sampleTime)) {
                continue
            }
            var x = safeLeftPadding + (sampleTime - safeScrollSeconds) * safePixelsPerSecond
            if (x < safeLeftPadding - 2 || x > width + 2) {
                continue
            }
            drawableSamples.push({
                "x": x,
                "intensity": root.clampedUnit(sample.intensity),
                "color": sample.color || "#93c5fd"
            })
        }
        if (drawableSamples.length === 0) {
            return
        }
        if (root.stripKind === "energy") {
            ctx.strokeStyle = root.energyColor
            ctx.beginPath()
            for (var energyIndex = 0; energyIndex < drawableSamples.length; energyIndex += 1) {
                var energySample = drawableSamples[energyIndex]
                ctx.moveTo(energySample.x, height)
                ctx.lineTo(energySample.x, height - energySample.intensity * height)
            }
            ctx.stroke()
        } else {
            for (var colorIndex = 0; colorIndex < drawableSamples.length; colorIndex += 1) {
                var colorSample = drawableSamples[colorIndex]
                var nextX = colorIndex + 1 < drawableSamples.length ? drawableSamples[colorIndex + 1].x : width
                var fillX = Math.max(safeLeftPadding, Math.min(width - 1, colorSample.x))
                var fillStop = Math.max(fillX + 1, Math.min(width, nextX))
                ctx.fillStyle = colorSample.color
                ctx.fillRect(fillX, 0, fillStop - fillX, height)
            }
        }
    }
}
