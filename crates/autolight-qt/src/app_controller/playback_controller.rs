use std::path::Path;

use autolight_core::graph::{find_track, source_track_id_for_context};
use autolight_core::project::{AudioAsset, ImportStatus};
use cxx_qt_lib::QString;

use super::{finite_non_negative, AppControllerState};

#[derive(Clone, Debug)]
pub(super) struct PlaybackControllerState {
    source_path: QString,
    position_seconds: f64,
    duration_seconds: f64,
    is_playing: bool,
    last_error: QString,
    volume: f64,
}

impl Default for PlaybackControllerState {
    fn default() -> Self {
        Self {
            source_path: QString::default(),
            position_seconds: 0.0,
            duration_seconds: 0.0,
            is_playing: false,
            last_error: QString::default(),
            volume: 1.0,
        }
    }
}

impl PlaybackControllerState {
    pub(super) fn source_path(&self) -> &QString {
        &self.source_path
    }

    pub(super) fn position_seconds(&self) -> f64 {
        self.position_seconds
    }

    pub(super) fn duration_seconds(&self) -> f64 {
        self.duration_seconds
    }

    pub(super) fn is_playing(&self) -> bool {
        self.is_playing
    }

    pub(super) fn last_error(&self) -> &QString {
        &self.last_error
    }

    pub(super) fn volume(&self) -> f64 {
        self.volume
    }

    fn source_path_string(&self) -> String {
        self.source_path.to_string()
    }

    fn load_source(&mut self, path: &str, duration_seconds: f64) {
        self.source_path = QString::from(path);
        self.duration_seconds = finite_non_negative(duration_seconds);
        self.position_seconds = 0.0;
        self.is_playing = false;
        self.last_error = QString::default();
    }

    fn unload(&mut self) {
        *self = Self::default();
    }

    fn fail_load(&mut self, path: &str) {
        self.unload();
        self.last_error = QString::from(&format!("audio file not found: {path}"));
    }

    fn play(&mut self) {
        self.is_playing = true;
        self.last_error = QString::default();
    }

    fn fail_empty_play(&mut self) {
        self.last_error = QString::from("no audio source loaded");
    }

    fn pause(&mut self) {
        self.is_playing = false;
    }

    fn stop(&mut self) {
        self.is_playing = false;
        self.position_seconds = 0.0;
    }

    fn seek(&mut self, seconds: f64) -> f64 {
        let position = finite_non_negative(seconds).min(self.duration_seconds.max(0.0));
        self.position_seconds = position;
        position
    }

    fn set_volume(&mut self, value: f64) {
        self.volume = finite_non_negative(value).clamp(0.0, 1.0);
    }
}

impl AppControllerState {
    pub(super) fn sync_playback_bridge_state(&mut self) {
        self.playback_source_path = self.playback.source_path().clone();
        self.playback_position_seconds = self.playback.position_seconds();
        self.playback_duration_seconds = self.playback.duration_seconds();
        self.playback_is_playing = self.playback.is_playing();
        self.playback_last_error = self.playback.last_error().clone();
        self.playback_volume = self.playback.volume();
    }

    pub(super) fn play_selected_track_state(&mut self) -> bool {
        let selected_track_id = self.selected_track_id.to_string();
        let Some(asset) = self
            .source_audio_asset_for_track_id(&selected_track_id)
            .cloned()
        else {
            self.set_error("selected track has no source audio");
            self.refresh_selected_state();
            return false;
        };
        if asset.import_status != ImportStatus::Online {
            self.set_error(format!("source audio is {}", asset.import_status));
            self.refresh_selected_state();
            return false;
        }
        if self.playback.source_path_string() != asset.path
            && !self.load_playback_source(&asset.path, asset.duration)
        {
            self.set_error(self.playback_last_error.to_string());
            self.refresh_selected_state();
            return false;
        }
        self.playback.play();
        self.sync_playback_bridge_state();
        self.last_error = QString::default();
        self.refresh_selected_state();
        true
    }

    pub(super) fn play_loaded_playback_state(&mut self) -> bool {
        if self.playback.source_path_string().is_empty() {
            self.playback.fail_empty_play();
            self.sync_playback_bridge_state();
            self.refresh_selected_state();
            return false;
        }
        self.playback.play();
        self.sync_playback_bridge_state();
        self.refresh_selected_state();
        true
    }

    pub(super) fn pause_playback_state(&mut self) {
        self.playback.pause();
        self.sync_playback_bridge_state();
        self.refresh_selected_state();
    }

    pub(super) fn stop_playback_state(&mut self) {
        self.playback.stop();
        self.sync_playback_bridge_state();
        self.refresh_selected_state();
    }

    pub(super) fn seek_playback_state(&mut self, seconds: f64) {
        let position = self.playback.seek(seconds);
        self.sync_playback_bridge_state();
        self.keep_timeline_time_visible(position);
        self.refresh_selected_state();
    }

    pub(super) fn nudge_playback_state(&mut self, delta_seconds: f64) {
        self.seek_playback_state(self.playback.position_seconds() + delta_seconds);
    }

    pub(super) fn set_playback_volume_state(&mut self, value: f64) {
        self.playback.set_volume(value);
        self.sync_playback_bridge_state();
        self.refresh_selected_state();
    }

    pub(super) fn source_audio_asset_for_track_id(&self, track_id: &str) -> Option<&AudioAsset> {
        let source_track_id = source_track_id_for_context(&self.project, track_id)?;
        let source_track = find_track(&self.project, &source_track_id)?;
        let asset_id = source_track
            .provenance
            .get("asset_id")
            .and_then(serde_json::Value::as_str)?;
        self.project
            .audio_assets
            .iter()
            .find(|asset| asset.id == asset_id)
    }

    pub(super) fn load_playback_source(&mut self, path: &str, duration_seconds: f64) -> bool {
        if !Path::new(path).is_file() {
            self.playback.fail_load(path);
            self.sync_playback_bridge_state();
            return false;
        }
        self.playback.load_source(path, duration_seconds);
        self.sync_playback_bridge_state();
        true
    }

    pub(super) fn unload_playback(&mut self) {
        self.playback.unload();
        self.sync_playback_bridge_state();
    }
}
