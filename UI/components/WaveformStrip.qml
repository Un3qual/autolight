import QtQuick

Canvas {
    id: root
    property var samples: []
    property real durationSeconds: 0
    property real scrollSeconds: 0
    property real pixelsPerSecond: 96
    property real leftPadding: 24
    property color peakColor: "#60a5fa"
    property color rmsColor: "#bfdbfe"

    function finiteNumber(value, fallbackValue) {
        var numericValue = Number(value)
        return isFinite(numericValue) ? numericValue : fallbackValue
    }

    function clampedUnit(value) {
        return Math.max(0, Math.min(1, root.finiteNumber(value, 0)))
    }

    onSamplesChanged: requestPaint()
    onScrollSecondsChanged: requestPaint()
    onPixelsPerSecondChanged: requestPaint()
    onLeftPaddingChanged: requestPaint()
    onPeakColorChanged: requestPaint()
    onRmsColorChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()

    onPaint: {
        var ctx = getContext("2d")
        ctx.clearRect(0, 0, width, height)
        if (!samples || samples.length === 0) {
            return
        }
        var centerY = height / 2
        ctx.strokeStyle = rmsColor
        ctx.lineWidth = 1
        var safeScrollSeconds = root.finiteNumber(scrollSeconds, 0)
        var safePixelsPerSecond = Math.max(0, root.finiteNumber(pixelsPerSecond, 96))
        var safeLeftPadding = root.finiteNumber(leftPadding, 24)
        var waveformHeight = Math.max(1, height - 18)
        var drawableSamples = []
        for (var i = 0; i < samples.length; i++) {
            var sample = samples[i]
            if (!sample || typeof sample !== "object") {
                continue
            }
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
                "peakHeight": Math.max(1, root.clampedUnit(sample.peak) * waveformHeight),
                "rmsHeight": Math.max(1, root.clampedUnit(sample.rms) * waveformHeight)
            })
        }
        ctx.strokeStyle = peakColor
        ctx.beginPath()
        for (var peakIndex = 0; peakIndex < drawableSamples.length; peakIndex++) {
            var peakSample = drawableSamples[peakIndex]
            ctx.moveTo(peakSample.x, centerY - peakSample.peakHeight / 2)
            ctx.lineTo(peakSample.x, centerY + peakSample.peakHeight / 2)
        }
        ctx.stroke()
        ctx.strokeStyle = rmsColor
        ctx.beginPath()
        for (var rmsIndex = 0; rmsIndex < drawableSamples.length; rmsIndex++) {
            var rmsSample = drawableSamples[rmsIndex]
            ctx.moveTo(rmsSample.x + 1, centerY - rmsSample.rmsHeight / 2)
            ctx.lineTo(rmsSample.x + 1, centerY + rmsSample.rmsHeight / 2)
        }
        ctx.stroke()
    }
}
