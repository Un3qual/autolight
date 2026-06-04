import QtQuick

Canvas {
    id: root
    property var levels: []
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

    function listOrEmpty(value) {
        return value && value.length !== undefined ? value : []
    }

    function normalizedLevel(level) {
        if (!level || typeof level !== "object") {
            return null
        }
        var levelSamples = root.listOrEmpty(level.samples)
        if (levelSamples.length === 0) {
            return null
        }
        var bucketCount = Math.round(root.finiteNumber(level.bucketCount, levelSamples.length))
        if (bucketCount <= 0 || bucketCount !== levelSamples.length) {
            bucketCount = levelSamples.length
        }
        return {
            "bucketCount": bucketCount,
            "samples": levelSamples
        }
    }

    function waveformLevels() {
        var sourceLevels = root.listOrEmpty(levels)
        var candidates = []
        for (var index = 0; index < sourceLevels.length; index++) {
            var level = root.normalizedLevel(sourceLevels[index])
            if (level) {
                candidates.push(level)
            }
        }
        candidates.sort(function(left, right) { return left.bucketCount - right.bucketCount })
        return candidates
    }

    function targetBucketCount() {
        var safePixelsPerSecond = Math.max(0, root.finiteNumber(pixelsPerSecond, 96))
        var drawableWidth = Math.max(0, width - root.finiteNumber(leftPadding, 24))
        var visibleSeconds = safePixelsPerSecond > 0 ? drawableWidth / safePixelsPerSecond : 0
        return Math.max(1, Math.ceil(visibleSeconds * safePixelsPerSecond / 8))
    }

    function selectedLevelPair() {
        var candidates = root.waveformLevels()
        if (candidates.length === 0) {
            return null
        }
        var target = root.targetBucketCount()
        if (target <= candidates[0].bucketCount) {
            return { "lower": candidates[0], "upper": candidates[0], "blend": 0 }
        }
        for (var index = 1; index < candidates.length; index++) {
            var upper = candidates[index]
            if (target <= upper.bucketCount) {
                var lower = candidates[index - 1]
                var span = Math.max(1, upper.bucketCount - lower.bucketCount)
                var blend = Math.max(0, Math.min(1, (target - lower.bucketCount) / span))
                return { "lower": lower, "upper": upper, "blend": blend }
            }
        }
        var finest = candidates[candidates.length - 1]
        return { "lower": finest, "upper": finest, "blend": 0 }
    }

    function sampleTime(sample, index, bucketCount, duration) {
        var explicitTime = root.finiteNumber(sample.time, NaN)
        if (isFinite(explicitTime)) {
            return explicitTime
        }
        if (bucketCount <= 0 || duration <= 0) {
            return 0
        }
        return index * duration / bucketCount
    }

    function drawWaveformLevel(ctx, level, alpha) {
        if (!level || alpha <= 0) {
            return
        }
        var safeScrollSeconds = root.finiteNumber(scrollSeconds, 0)
        var safePixelsPerSecond = Math.max(0, root.finiteNumber(pixelsPerSecond, 96))
        var safeLeftPadding = root.finiteNumber(leftPadding, 24)
        var safeDurationSeconds = Math.max(0, root.finiteNumber(durationSeconds, 0))
        var centerY = height / 2
        var waveformHeight = Math.max(1, height - 18)
        var bucketWidth = Math.max(
            1,
            level.bucketCount > 0 && safeDurationSeconds > 0
                ? safeDurationSeconds / level.bucketCount * safePixelsPerSecond
                : 1
        )
        var drawableSamples = []
        for (var i = 0; i < level.samples.length; i++) {
            var sample = level.samples[i]
            if (!sample || typeof sample !== "object") {
                continue
            }
            var sampleTime = root.sampleTime(sample, i, level.bucketCount, safeDurationSeconds)
            if (!isFinite(sampleTime)) {
                continue
            }
            var x = safeLeftPadding + (sampleTime - safeScrollSeconds) * safePixelsPerSecond
            if (x + bucketWidth < safeLeftPadding - 2 || x > width + 2) {
                continue
            }
            var drawX = Math.max(safeLeftPadding, x)
            var drawStop = Math.min(width, x + bucketWidth)
            var drawWidth = Math.max(1, drawStop - drawX)
            var peakHeight = Math.max(1, root.clampedUnit(sample.peak) * waveformHeight)
            var rmsHeight = Math.max(1, root.clampedUnit(sample.rms) * waveformHeight)
            drawableSamples.push({
                "x": drawX,
                "width": drawWidth,
                "peakTop": centerY - peakHeight / 2,
                "peakHeight": peakHeight,
                "rmsTop": centerY - rmsHeight / 2,
                "rmsHeight": rmsHeight
            })
        }
        var previousAlpha = ctx.globalAlpha
        ctx.globalAlpha = previousAlpha * alpha
        ctx.fillStyle = peakColor
        for (var peakIndex = 0; peakIndex < drawableSamples.length; peakIndex++) {
            var peakSample = drawableSamples[peakIndex]
            ctx.fillRect(peakSample.x, peakSample.peakTop, peakSample.width, peakSample.peakHeight)
        }
        ctx.fillStyle = rmsColor
        for (var rmsIndex = 0; rmsIndex < drawableSamples.length; rmsIndex++) {
            var rmsSample = drawableSamples[rmsIndex]
            ctx.fillRect(rmsSample.x, rmsSample.rmsTop, rmsSample.width, rmsSample.rmsHeight)
        }
        ctx.globalAlpha = previousAlpha
    }

    onLevelsChanged: requestPaint()
    onDurationSecondsChanged: requestPaint()
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
        var pair = root.selectedLevelPair()
        if (!pair) {
            return
        }
        if (pair.lower === pair.upper) {
            root.drawWaveformLevel(ctx, pair.lower, 1)
        } else {
            root.drawWaveformLevel(ctx, pair.lower, 1 - pair.blend)
            root.drawWaveformLevel(ctx, pair.upper, pair.blend)
        }
    }
}
