from __future__ import annotations

import math
from pathlib import Path

from PySide6.QtCore import Property, QObject, QUrl, Signal, Slot
from PySide6.QtMultimedia import QAudioOutput, QMediaPlayer


class PlaybackTransport(QObject):
    sourcePathChanged = Signal()
    positionSecondsChanged = Signal()
    durationSecondsChanged = Signal()
    isPlayingChanged = Signal()
    lastErrorChanged = Signal()
    volumeChanged = Signal()

    def __init__(self, *, player=None, audio_output=None, parent: QObject | None = None):
        super().__init__(parent)
        self._player = player if player is not None else QMediaPlayer(self)
        self._audio_output = audio_output if audio_output is not None else QAudioOutput(self)
        self._player.setAudioOutput(self._audio_output)
        self._source_path = ""
        self._position_seconds = 0.0
        self._duration_seconds = 0.0
        self._is_playing = False
        self._last_error = ""
        self._volume = 1.0
        self._player.positionChanged.connect(self._handle_position_changed)
        self._player.durationChanged.connect(self._handle_duration_changed)
        self._player.playbackStateChanged.connect(self._handle_playback_state_changed)
        self._player.errorOccurred.connect(self._handle_error)

    @Property(str, notify=sourcePathChanged)
    def sourcePath(self) -> str:
        return self._source_path

    @Property(float, notify=positionSecondsChanged)
    def positionSeconds(self) -> float:
        return self._position_seconds

    @Property(float, notify=durationSecondsChanged)
    def durationSeconds(self) -> float:
        return self._duration_seconds

    @Property(bool, notify=isPlayingChanged)
    def isPlaying(self) -> bool:
        return self._is_playing

    @Property(str, notify=lastErrorChanged)
    def lastError(self) -> str:
        return self._last_error

    @Property(float, notify=volumeChanged)
    def volume(self) -> float:
        return self._volume

    @Slot(str, float, result=bool)
    def load_source(self, path: str, duration_seconds: float = 0.0) -> bool:
        source_path = str(Path(path))
        if not Path(source_path).is_file():
            if self._source_path:
                self.unload()
            self._set_last_error(f"audio file not found: {source_path}")
            return False
        self.stop()
        self._player.setSource(QUrl.fromLocalFile(source_path))
        self._set_source_path(source_path)
        self._set_duration_seconds(self._finite_non_negative(duration_seconds))
        self._set_position_seconds(0.0)
        self._set_is_playing(False)
        self._set_last_error("")
        return True

    @Slot()
    def unload(self) -> None:
        self.stop()
        self._player.setSource(QUrl())
        self._set_source_path("")
        self._set_duration_seconds(0.0)
        self._set_position_seconds(0.0)
        self._set_last_error("")

    @Slot()
    def play(self) -> None:
        if not self._source_path:
            self._set_last_error("no audio source loaded")
            return
        self._player.play()

    @Slot()
    def pause(self) -> None:
        self._player.pause()

    @Slot()
    def stop(self) -> None:
        self._set_is_playing(False)
        self._player.stop()
        self._player.setPosition(0)
        self._set_position_seconds(0.0)

    @Slot(float)
    def seek_seconds(self, seconds: float) -> None:
        position = min(self._finite_non_negative(seconds), self._duration_seconds)
        self._player.setPosition(int(round(position * 1000.0)))
        self._set_position_seconds(position)

    @Slot(float)
    def set_volume(self, value: float) -> None:
        volume = min(max(self._finite_non_negative(value), 0.0), 1.0)
        self._audio_output.setVolume(volume)
        if self._volume == volume:
            return
        self._volume = volume
        self.volumeChanged.emit()

    def _handle_position_changed(self, milliseconds: int) -> None:
        self._set_position_seconds(self._finite_non_negative(milliseconds / 1000.0))

    def _handle_duration_changed(self, milliseconds: int) -> None:
        duration_seconds = self._finite_non_negative(milliseconds / 1000.0)
        if duration_seconds > 0.0 or self._duration_seconds <= 0.0:
            self._set_duration_seconds(duration_seconds)

    def _handle_playback_state_changed(self, state) -> None:
        playback_state = getattr(self._player, "PlaybackState", QMediaPlayer.PlaybackState)
        playing_state = getattr(
            playback_state, "PlayingState", QMediaPlayer.PlaybackState.PlayingState
        )
        self._set_is_playing(state == playing_state)

    def _handle_error(self, _error, message: str = "") -> None:
        self._set_is_playing(False)
        self._set_last_error(message or "media playback failed")

    def _set_source_path(self, value: str) -> None:
        if self._source_path == value:
            return
        self._source_path = value
        self.sourcePathChanged.emit()

    def _set_position_seconds(self, value: float) -> None:
        if self._position_seconds == value:
            return
        self._position_seconds = value
        self.positionSecondsChanged.emit()

    def _set_duration_seconds(self, value: float) -> None:
        if self._duration_seconds == value:
            return
        self._duration_seconds = value
        self.durationSecondsChanged.emit()

    def _set_is_playing(self, value: bool) -> None:
        if self._is_playing == value:
            return
        self._is_playing = value
        self.isPlayingChanged.emit()

    def _set_last_error(self, value: str) -> None:
        if self._last_error == value:
            return
        self._last_error = value
        self.lastErrorChanged.emit()

    @staticmethod
    def _finite_non_negative(value: float) -> float:
        number = float(value)
        return number if math.isfinite(number) and number >= 0.0 else 0.0
