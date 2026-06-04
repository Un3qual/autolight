import QtQml
import QtQml.Models
import QtMultimedia
import Autolight.Qt 1.0

QtObject {
    id: rustAdapter

    property var rustController: AppController {}
    readonly property string projectName: rustController.projectName
    readonly property string lastError: rustController.lastError
    readonly property string timelineRowsJson: rustController.timelineRowsJson
    readonly property string transformSpecsJson: rustController.transformSpecsJson
    readonly property string selectedMarkerIdsJson: rustController.selectedMarkerIdsJson
    readonly property string selectedTrackMarkersJson: rustController.selectedTrackMarkersJson
    readonly property string markerColorOptionsJson: rustController.markerColorOptionsJson
    readonly property string projectPath: rustController.projectPath
    readonly property bool isDirty: rustController.isDirty
    readonly property bool canUndo: rustController.canUndo
    readonly property bool canRedo: rustController.canRedo
    readonly property string selectedTrackId: rustController.selectedTrackId
    readonly property bool selectedTrackCanPlay: rustController.selectedTrackCanPlay
    readonly property bool selectedTrackCanRerun: rustController.selectedTrackCanRerun
    readonly property bool selectedTrackHasRunningJob: rustController.selectedTrackHasRunningJob
    readonly property bool selectedTrackIsEditable: rustController.selectedTrackIsEditable
    property var selectedMarkerIds: []
    property var selectedTrackMarkers: []
    property var markerColorOptions: []
    readonly property real timelinePixelsPerSecond: rustController.timelinePixelsPerSecond
    readonly property real timelineScrollSeconds: rustController.timelineScrollSeconds
    readonly property real timelineVisibleSeconds: rustController.timelineVisibleSeconds
    readonly property real timelineDurationSeconds: rustController.timelineDurationSeconds

    property var trackModel: ListModel {}
    property var transformModel: ListModel {
        function version_at(index) {
            if (index < 0 || index >= count) return ""
            return get(index).version
        }
    }
    property var audioOutput: AudioOutput { volume: rustController.playbackVolume }
    property var mediaPlayer: MediaPlayer {
        audioOutput: rustAdapter.audioOutput
    }
    property var playback: QtObject {
        readonly property string lastError: rustController.playbackLastError.length > 0 ? rustController.playbackLastError : rustAdapter.mediaPlayer.errorString
        readonly property bool isPlaying: rustAdapter.mediaPlayer.playbackState === MediaPlayer.PlayingState
        readonly property string sourcePath: rustController.playbackSourcePath
        readonly property real positionSeconds: rustAdapter.mediaPlayer.source.toString().length > 0 ? rustAdapter.mediaPlayer.position / 1000.0 : rustController.playbackPositionSeconds
        readonly property real durationSeconds: rustAdapter.mediaPlayer.duration > 0 ? rustAdapter.mediaPlayer.duration / 1000.0 : rustController.playbackDurationSeconds
        readonly property real volume: rustAdapter.audioOutput.volume

        function play() {
            var played = rustController.playLoadedPlayback()
            rustAdapter.reloadModels()
            if (played && rustAdapter.syncPlaybackSource()) rustAdapter.mediaPlayer.play()
            return played
        }

        function set_volume(value) {
            rustController.setPlaybackVolumeValue(value)
            rustAdapter.audioOutput.volume = rustController.playbackVolume
            rustAdapter.reloadModels()
        }
    }

    function playbackSourceUrl(path) {
        if (path.length === 0) return ""
        if (path.indexOf("file://") === 0) return path
        var encodedPath = path.split("/").map(function(segment) { return encodeURIComponent(segment) }).join("/")
        return "file://" + encodedPath
    }

    function syncPlaybackSource() {
        var path = rustController.playbackSourcePath
        if (path.length === 0) {
            mediaPlayer.stop()
            mediaPlayer.source = ""
            return false
        }
        var sourceUrl = playbackSourceUrl(path)
        if (mediaPlayer.source.toString() !== sourceUrl) mediaPlayer.source = sourceUrl
        return true
    }

    function reloadTrackModel() {
        trackModel.clear()
        var rows = []
        try {
            rows = JSON.parse(rustController.timelineRowsJson)
        } catch (error) {
            console.error("Failed to parse timelineRowsJson:", error, rustController.timelineRowsJson)
            return
        }
        for (var i = 0; i < rows.length; i++) {
            trackModel.append(rows[i])
        }
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
            rows = JSON.parse(rustController.transformSpecsJson)
        } catch (error) {
            console.error("Failed to parse transformSpecsJson:", error, rustController.transformSpecsJson)
            return
        }
        for (var i = 0; i < rows.length; i++) {
            transformModel.append(rows[i])
        }
    }

    function reloadSelectionModels() {
        selectedMarkerIds = parseJsonArray(rustController.selectedMarkerIdsJson)
        selectedTrackMarkers = parseJsonArray(rustController.selectedTrackMarkersJson)
        markerColorOptions = parseJsonArray(rustController.markerColorOptionsJson)
    }

    function reloadModels() {
        reloadTrackModel()
        reloadTransformModel()
        reloadSelectionModels()
        syncPlaybackSource()
        audioOutput.volume = rustController.playbackVolume
    }
    function new_project() { rustController.newProject(); reloadModels() }
    function open_project(path) { var opened = rustController.openProject(path); reloadModels(); return opened }
    function save_project(path) { var saved = rustController.saveProject(path || ""); reloadModels(); return saved }
    function import_audio(path) { var id = rustController.importAudio(path); reloadModels(); return id }
    function load_demo_project() { rustController.loadDemoProject(); reloadModels() }
    function add_manual_cue_track(name) { var id = rustController.addManualCueTrack(name || "Manual Cues"); reloadModels(); return id }
    function undo() { var changed = rustController.undo(); reloadModels(); return changed }
    function redo() { var changed = rustController.redo(); reloadModels(); return changed }
    function add_fixed_interval_track(trackId, duration, interval) { return add_transform_track(trackId, "markers.fixed_interval", "1", JSON.stringify({"duration": duration, "interval": interval})) }
    function run_track(trackId) { var id = rustController.runTrack(trackId); reloadModels(); return id }
    function rerun_track(trackId) { var id = rustController.rerunTrack(trackId); reloadModels(); return id }
    function cancel_selected_job() { rustController.cancelSelectedJob(); reloadModels() }
    function add_transform_track(trackId, transformId, transformVersion, params) { var id = rustController.addTransformTrack(trackId, transformId, transformVersion, params); reloadModels(); return id }
    function add_vocals_stem_track(trackId) { return add_transform_track(trackId, "stems.vocals_stand_in", "1", "{}") }
    function refresh_cache_status() { var refs = rustController.refreshCacheStatus(); reloadModels(); return refs }
    function create_editable_track_from_track(trackId) { var id = rustController.createEditableTrackFromTrack(trackId); reloadModels(); return id }
    function pause_playback() { rustController.pausePlayback(); mediaPlayer.pause(); reloadModels() }
    function play_selected_track() { var played = rustController.playSelectedTrack(); reloadModels(); if (played && syncPlaybackSource()) mediaPlayer.play(); return played }
    function stop_playback() { rustController.stopPlayback(); mediaPlayer.stop(); mediaPlayer.seek(0); reloadModels() }
    function nudge_playback(delta) { seek_playback(playback.positionSeconds + delta) }
    function seek_playback(value) { rustController.seekPlayback(value); reloadModels(); if (syncPlaybackSource()) mediaPlayer.seek(rustController.playbackPositionSeconds * 1000) }
    function set_timeline_zoom(value) { rustController.setTimelineZoom(value); reloadModels() }
    function set_timeline_scroll_seconds(value) { rustController.applyTimelineScrollSeconds(value); reloadModels() }
    function set_timeline_visible_seconds(value) { rustController.applyTimelineVisibleSeconds(value); reloadModels() }
    function set_timeline_visible_track_range(firstRow, rowCount) { rustController.setTimelineVisibleTrackRange(firstRow, rowCount) }
    function select_track(trackId) { rustController.selectTrack(trackId); reloadModels() }
    function set_track_expanded(trackId, expanded) { var changed = rustController.setTrackExpanded(trackId, expanded); reloadModels(); return changed }
    function snap_timeline_time(seconds, bypassSnap) { return rustController.snapTimelineTime(seconds, bypassSnap) }
    function add_marker_to_selected_track_with_duration(timestamp, duration, label, category, colorKey) { var id = rustController.addMarkerToSelectedTrackWithDuration(timestamp, duration, label, category, colorKey); reloadModels(); return id }
    function delete_marker_from_selected_track(markerId) { var deleted = rustController.deleteMarkerFromSelectedTrack(markerId); reloadModels(); return deleted }
    function delete_selected_markers() { var deleted = rustController.deleteSelectedMarkers(); reloadModels(); return deleted }
    function update_selected_marker_with_duration(timestamp, duration, label, category, colorKey) { var updated = rustController.updateSelectedMarkerWithDuration(timestamp, duration, label, category, colorKey); reloadModels(); return updated }
    function bulk_update_selected_markers(label, category, colorKey) { var updated = rustController.bulkUpdateSelectedMarkers(label, category, colorKey); reloadModels(); return updated }
    function toggle_marker_selection(markerId, extendSelection) { rustController.toggleMarkerSelection(markerId, extendSelection); reloadModels() }
    function move_selected_markers(delta, bypass) { var moved = rustController.moveSelectedMarkers(delta, bypass); reloadModels(); return moved }
    function resize_marker(markerId, duration) { var resized = rustController.resizeMarker(markerId, duration); reloadModels(); return resized }

    Component.onCompleted: load_demo_project()
}
