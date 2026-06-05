import QtQuick
import QtQml
import QtQml.Models
import QtMultimedia
import Autolight.Qt 1.0

QtObject {
    id: appRuntime

    property var nativeController: AppController {}
    readonly property string projectName: nativeController.projectName
    readonly property string lastError: nativeController.lastError
    readonly property string timelineRowsJson: nativeController.timelineRowsJson
    property string timelineSceneSnapshotJson: nativeController.timelineSceneSnapshotJson
    readonly property string transformSpecsJson: nativeController.transformSpecsJson
    readonly property string selectedMarkerIdsJson: nativeController.selectedMarkerIdsJson
    readonly property string selectedTrackMarkersJson: nativeController.selectedTrackMarkersJson
    readonly property string markerColorOptionsJson: nativeController.markerColorOptionsJson
    readonly property string projectPath: nativeController.projectPath
    readonly property bool isDirty: nativeController.isDirty
    readonly property bool canUndo: nativeController.canUndo
    readonly property bool canRedo: nativeController.canRedo
    property string selectedTrackId: nativeController.selectedTrackId
    property bool selectedTrackCanPlay: nativeController.selectedTrackCanPlay
    property bool selectedTrackCanRerun: nativeController.selectedTrackCanRerun
    property bool selectedTrackHasRunningJob: nativeController.selectedTrackHasRunningJob
    property bool selectedTrackIsEditable: nativeController.selectedTrackIsEditable
    property var selectedMarkerIds: []
    property var selectedTrackMarkers: []
    property var markerColorOptions: []
    property real timelinePixelsPerSecond: nativeController.timelinePixelsPerSecond
    property real timelineMinPixelsPerSecond: nativeController.timelineMinPixelsPerSecond
    property real timelineMaxPixelsPerSecond: nativeController.timelineMaxPixelsPerSecond
    property real timelineScrollSeconds: nativeController.timelineScrollSeconds
    property real timelineVisibleSeconds: nativeController.timelineVisibleSeconds
    property real timelineDurationSeconds: nativeController.timelineDurationSeconds
    property int timelineFollowMode: nativeController.timelineFollowMode
    property bool timelineUserNavigationActive: nativeController.timelineUserNavigationActive
    property int timelinePlayheadOffscreenDirection: nativeController.timelinePlayheadOffscreenDirection
    readonly property bool smoothTimelineFollow: nativeController.playbackIsPlaying && timelineFollowMode !== 0 && !timelineUserNavigationActive

    Behavior on timelineScrollSeconds {
        enabled: appRuntime.smoothTimelineFollow
        SmoothedAnimation {
            velocity: 1.0
            maximumEasingTime: 80
        }
    }

    property var trackRows: []
    property var transformModel: ListModel {
        function version_at(index) {
            if (index < 0 || index >= count) return ""
            return get(index).version
        }
    }
    property var audioOutput: AudioOutput { volume: nativeController.playbackVolume }
    property var mediaPlayer: MediaPlayer {
        audioOutput: appRuntime.audioOutput
        onPositionChanged: function(position) {
            if (source.toString().length > 0) {
                nativeController.syncPlaybackPosition(position / 1000.0)
                reloadViewportState()
            }
        }
    }
    property var jobPollTimer: Timer {
        interval: 80
        repeat: true
        running: appRuntime.selectedTrackHasRunningJob
            || appRuntime.trackRows.some(function(row) { return row.activeJobId && row.activeJobId.length > 0 })
        onTriggered: appRuntime.poll_jobs()
    }
    property var playback: QtObject {
        readonly property string lastError: nativeController.playbackLastError.length > 0 ? nativeController.playbackLastError : appRuntime.mediaPlayer.errorString
        readonly property bool isPlaying: appRuntime.mediaPlayer.playbackState === MediaPlayer.PlayingState
        readonly property string sourcePath: nativeController.playbackSourcePath
        readonly property real positionSeconds: appRuntime.mediaPlayer.source.toString().length > 0 ? appRuntime.mediaPlayer.position / 1000.0 : nativeController.playbackPositionSeconds
        readonly property real durationSeconds: appRuntime.mediaPlayer.duration > 0 ? appRuntime.mediaPlayer.duration / 1000.0 : nativeController.playbackDurationSeconds
        readonly property real volume: appRuntime.audioOutput.volume

        function play() {
            var played = nativeController.playLoadedPlayback()
            appRuntime.reloadModels()
            if (played && appRuntime.syncPlaybackSource()) appRuntime.mediaPlayer.play()
            return played
        }

        function set_volume(value) {
            nativeController.setPlaybackVolumeValue(value)
            appRuntime.audioOutput.volume = nativeController.playbackVolume
            appRuntime.reloadModels()
        }
    }

    function playbackSourceUrl(path) {
        if (path.length === 0) return ""
        if (path.indexOf("file://") === 0) return path
        var normalizedPath = path.replace(/\\/g, "/")
        var encodedPath = normalizedPath.split("/").map(function(segment, index) {
            if (index === 0 && segment.match(/^[A-Za-z]:$/)) return segment
            return encodeURIComponent(segment)
        }).join("/")
        if (normalizedPath.match(/^[A-Za-z]:\//)) return "file:///" + encodedPath
        if (normalizedPath.indexOf("//") === 0) return "file:" + encodedPath
        return "file://" + encodedPath
    }

    function syncPlaybackSource() {
        var path = nativeController.playbackSourcePath
        if (path.length === 0) {
            mediaPlayer.stop()
            mediaPlayer.source = ""
            return false
        }
        var sourceUrl = playbackSourceUrl(path)
        if (mediaPlayer.source.toString() !== sourceUrl) mediaPlayer.source = sourceUrl
        return true
    }

    function seekMediaPlayerToSeconds(seconds) {
        var positionMs = Math.max(0, Number(seconds) * 1000)
        if (!isFinite(positionMs)) positionMs = 0
        mediaPlayer.position = positionMs
    }

    function reloadTrackModel() {
        var rows = []
        try {
            rows = JSON.parse(nativeController.timelineRowsJson)
        } catch (error) {
            console.error("Failed to parse timelineRowsJson:", error, nativeController.timelineRowsJson)
            trackRows = []
            return
        }
        trackRows = rows
    }

    function reloadTimelineSceneSnapshot() {
        timelineSceneSnapshotJson = nativeController.timelineSceneSnapshotJson
    }

    function parseJsonArray(payload) {
        try {
            var rows = JSON.parse(payload)
            return Array.isArray(rows) ? rows : []
        } catch (error) {
            return []
        }
    }

    function reloadTransformModel() {
        transformModel.clear()
        var rows = []
        try {
            rows = JSON.parse(nativeController.transformSpecsJson)
        } catch (error) {
            console.error("Failed to parse transformSpecsJson:", error, nativeController.transformSpecsJson)
            return
        }
        for (var i = 0; i < rows.length; i++) {
            if (rows[i].runnable === true) {
                transformModel.append(rows[i])
            }
        }
    }

    function reloadSelectionModels() {
        selectedTrackId = nativeController.selectedTrackId
        selectedTrackCanPlay = nativeController.selectedTrackCanPlay
        selectedTrackCanRerun = nativeController.selectedTrackCanRerun
        selectedTrackHasRunningJob = nativeController.selectedTrackHasRunningJob
        selectedTrackIsEditable = nativeController.selectedTrackIsEditable
        selectedMarkerIds = parseJsonArray(nativeController.selectedMarkerIdsJson)
        selectedTrackMarkers = parseJsonArray(nativeController.selectedTrackMarkersJson)
        markerColorOptions = parseJsonArray(nativeController.markerColorOptionsJson)
    }

    function reloadViewportState() {
        timelinePixelsPerSecond = nativeController.timelinePixelsPerSecond
        timelineMinPixelsPerSecond = nativeController.timelineMinPixelsPerSecond
        timelineMaxPixelsPerSecond = nativeController.timelineMaxPixelsPerSecond
        timelineScrollSeconds = nativeController.timelineScrollSeconds
        timelineVisibleSeconds = nativeController.timelineVisibleSeconds
        timelineDurationSeconds = nativeController.timelineDurationSeconds
        timelineFollowMode = nativeController.timelineFollowMode
        timelineUserNavigationActive = nativeController.timelineUserNavigationActive
        timelinePlayheadOffscreenDirection = nativeController.timelinePlayheadOffscreenDirection
    }

    function reloadModels() {
        reloadSelectionModels()
        reloadViewportState()
        reloadTrackModel()
        reloadTimelineSceneSnapshot()
        reloadTransformModel()
        syncPlaybackSource()
        audioOutput.volume = nativeController.playbackVolume
    }
    function start_default_project() { load_demo_project() }
    function new_project() { nativeController.newProject(); reloadModels() }
    function open_project(path) { var opened = nativeController.openProject(path); reloadModels(); return opened }
    function save_project(path) { var saved = nativeController.saveProject(path || ""); reloadModels(); return saved }
    function import_audio(path) { var id = nativeController.importAudio(path); reloadModels(); return id }
    function load_demo_project() { nativeController.loadDemoProject(); reloadModels() }
    function add_manual_cue_track(name) { var id = nativeController.addManualCueTrack(name || "Manual Cues"); reloadModels(); return id }
    function undo() { var changed = nativeController.undo(); reloadModels(); return changed }
    function redo() { var changed = nativeController.redo(); reloadModels(); return changed }
    function add_fixed_interval_track(trackId, duration, interval) { return add_transform_track(trackId, "markers.fixed_interval", "1", JSON.stringify({"duration": duration, "interval": interval})) }
    function run_track(trackId) { var id = nativeController.runTrack(trackId); reloadModels(); return id }
    function rerun_track(trackId) { var id = nativeController.rerunTrack(trackId); reloadModels(); return id }
    function cancel_selected_job() { nativeController.cancelSelectedJob(); reloadModels() }
    function poll_jobs() {
        var changed = nativeController.pollJobs()
        if (changed > 0) reloadModels()
        return changed
    }
    function add_transform_track(trackId, transformId, transformVersion, params) { var id = nativeController.addTransformTrack(trackId, transformId, transformVersion, params); reloadModels(); return id }
    function refresh_cache_status() { var refs = nativeController.refreshCacheStatus(); reloadModels(); return refs }
    function create_editable_track_from_track(trackId) { var id = nativeController.createEditableTrackFromTrack(trackId); reloadModels(); return id }
    function pause_playback() { nativeController.pausePlayback(); mediaPlayer.pause(); reloadModels() }
    function play_selected_track() { var played = nativeController.playSelectedTrack(); reloadModels(); if (played && syncPlaybackSource()) mediaPlayer.play(); return played }
    function stop_playback() { nativeController.stopPlayback(); mediaPlayer.stop(); seekMediaPlayerToSeconds(0); reloadModels() }
    function nudge_playback(delta) { seek_playback(playback.positionSeconds + delta) }
    function seek_playback(value) { nativeController.seekPlayback(value); reloadModels(); if (syncPlaybackSource()) seekMediaPlayerToSeconds(nativeController.playbackPositionSeconds) }
    function set_timeline_zoom(value) {
        nativeController.setTimelineZoom(value)
        reloadViewportState()
    }
    function set_timeline_zoom_for_lane_width(value, laneWidth) {
        nativeController.setTimelineZoomForLaneWidth(value, laneWidth)
        reloadViewportState()
    }
    function set_timeline_scroll_seconds(value) {
        nativeController.applyTimelineScrollSeconds(value)
        reloadViewportState()
    }
    function set_timeline_visible_seconds(value) {
        nativeController.applyTimelineVisibleSeconds(value)
        reloadViewportState()
    }
    function set_timeline_visible_lane_width(laneWidth) {
        nativeController.setTimelineVisibleLaneWidth(laneWidth)
        reloadViewportState()
    }
    function fit_timeline_to_lane_width(laneWidth) {
        nativeController.fitTimelineToLaneWidth(laneWidth)
        reloadViewportState()
    }
    function scroll_timeline_by_pixels(pixelDelta) {
        nativeController.scrollTimelineByPixels(pixelDelta)
        reloadViewportState()
    }
    function zoom_timeline_by_factor(factor, anchorX, laneWidth) {
        nativeController.zoomTimelineByFactor(factor, anchorX, laneWidth)
        reloadViewportState()
    }
    function begin_timeline_user_navigation() {
        nativeController.beginTimelineUserNavigation()
        reloadViewportState()
    }
    function end_timeline_user_navigation() {
        nativeController.endTimelineUserNavigation()
        reloadViewportState()
    }
    function scrub_timeline_at_x(x, laneWidth) {
        var seconds = nativeController.scrubTimelineAtX(x, laneWidth)
        reloadViewportState()
        if (syncPlaybackSource()) seekMediaPlayerToSeconds(nativeController.playbackPositionSeconds)
        return seconds
    }
    function set_timeline_follow_mode(mode) {
        nativeController.applyTimelineFollowMode(mode)
        reloadViewportState()
    }
    function set_timeline_visible_track_range(firstRow, rowCount) {
        nativeController.setTimelineVisibleTrackRange(firstRow, rowCount)
        reloadViewportState()
    }
    function select_track(trackId) {
        nativeController.selectTrack(trackId)
        reloadSelectionModels()
        reloadTrackModel()
        reloadTimelineSceneSnapshot()
    }
    function set_track_expanded(trackId, expanded) { var changed = nativeController.setTrackExpanded(trackId, expanded); reloadModels(); return changed }
    function snap_timeline_time(seconds, bypassSnap) { return nativeController.snapTimelineTime(seconds, bypassSnap) }
    function add_marker_to_selected_track_with_duration(timestamp, duration, label, category, colorKey) { var id = nativeController.addMarkerToSelectedTrackWithDuration(timestamp, duration, label, category, colorKey); reloadModels(); return id }
    function delete_marker_from_selected_track(markerId) { var deleted = nativeController.deleteMarkerFromSelectedTrack(markerId); reloadModels(); return deleted }
    function delete_selected_markers() { var deleted = nativeController.deleteSelectedMarkers(); reloadModels(); return deleted }
    function update_selected_marker_with_duration(timestamp, duration, label, category, colorKey) { var updated = nativeController.updateSelectedMarkerWithDuration(timestamp, duration, label, category, colorKey); reloadModels(); return updated }
    function bulk_update_selected_markers(label, category, colorKey) { var updated = nativeController.bulkUpdateSelectedMarkers(label, category, colorKey); reloadModels(); return updated }
    function toggle_marker_selection(markerId, extendSelection) { nativeController.toggleMarkerSelection(markerId, extendSelection); reloadModels() }
    function move_selected_markers(delta, bypass) { var moved = nativeController.moveSelectedMarkers(delta, bypass); reloadModels(); return moved }
    function resize_marker(markerId, duration) { var resized = nativeController.resizeMarker(markerId, duration); reloadModels(); return resized }

}
