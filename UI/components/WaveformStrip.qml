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

    onSamplesChanged: requestPaint()
    onScrollSecondsChanged: requestPaint()
    onPixelsPerSecondChanged: requestPaint()
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
        for (var i = 0; i < samples.length; i++) {
            var sample = samples[i]
            var x = leftPadding + (sample.time - scrollSeconds) * pixelsPerSecond
            if (x < leftPadding - 2 || x > width + 2) {
                continue
            }
            var peakHeight = Math.max(1, sample.peak * (height - 18))
            var rmsHeight = Math.max(1, sample.rms * (height - 18))
            ctx.strokeStyle = peakColor
            ctx.beginPath()
            ctx.moveTo(x, centerY - peakHeight / 2)
            ctx.lineTo(x, centerY + peakHeight / 2)
            ctx.stroke()
            ctx.strokeStyle = rmsColor
            ctx.beginPath()
            ctx.moveTo(x + 1, centerY - rmsHeight / 2)
            ctx.lineTo(x + 1, centerY + rmsHeight / 2)
            ctx.stroke()
        }
    }
}
