import QtQuick

Canvas {
    id: root
    required property var samples
    required property string stripKind
    property real durationSeconds: 0
    property color energyColor: "#facc15"
    width: parent ? parent.width : 0
    height: 16
    visible: root.sampleCount() > 0

    function sampleCount() {
        return root.samples && root.samples.length ? root.samples.length : 0
    }

    onSamplesChanged: requestPaint()
    onStripKindChanged: requestPaint()
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
        for (var index = 0; index < count; index += 1) {
            var sample = root.samples[index] || {}
            var x = index / Math.max(1, count - 1) * width
            if (root.stripKind === "energy") {
                var intensity = Math.max(0, Math.min(1, Number(sample.intensity || 0)))
                ctx.strokeStyle = root.energyColor
                ctx.beginPath()
                ctx.moveTo(x, height)
                ctx.lineTo(x, height - intensity * height)
                ctx.stroke()
            } else {
                ctx.fillStyle = sample.color || "#93c5fd"
                ctx.fillRect(x, 0, Math.max(1, width / count), height)
            }
        }
    }
}
