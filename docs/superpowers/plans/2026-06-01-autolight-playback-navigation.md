# Autolight Playback And Timeline Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add source-audio playback, a visible playhead, and zoomable/pannable timeline navigation so users can inspect generated and editable markers against real time.

**Architecture:** Keep Python in charge of media playback state and project-derived timeline state. Add a small Qt `PlaybackTransport` wrapper around `QMediaPlayer`/`QAudioOutput`, expose it through `AppController`, and keep QML as a thin control and rendering layer. Timeline zoom and scroll state live on `AppController` so QML, tests, and future persistence all share one source of truth.

**Tech Stack:** Python 3.14, PySide6 `QtMultimedia`, PySide6/QML, `unittest`, existing `AppController`, `AudioAsset`, `TimelineTrackModel`, and `UI/Main.qml`.

---

## File Structure

- Create `autolight/playback/__init__.py`: export playback transport classes.
- Create `autolight/playback/transport.py`: wrap `QMediaPlayer` and `QAudioOutput` behind a QML-safe `QObject`.
- Create `tests/test_playback_transport.py`: unit-test transport state with fake player and audio output objects.
- Modify `autolight/app_controller.py`: own the transport, resolve selected tracks to playable source audio, expose timeline zoom/scroll/duration, and stop playback before project replacement.
- Modify `tests/test_app_controller.py`: cover selected-track playability, playback slots, viewport clamping, and QML wiring.
- Modify `UI/Main.qml`: add transport controls, scrubber, playhead overlay, zoom control, and horizontal timeline navigation.
- Modify `README.md`: document playback and navigation in the basic workflow.

## Task 1: Playback Transport Wrapper

**Files:**
- Create: `autolight/playback/__init__.py`
- Create: `autolight/playback/transport.py`
- Create: `tests/test_playback_transport.py`

- [x] **Step 1: Write failing playback transport tests**

Create `tests/test_playback_transport.py`:

```python
import unittest

from PySide6.QtCore import QCoreApplication, QUrl

from autolight.playback.transport import PlaybackTransport


class FakeAudioOutput:
    def __init__(self):
        self.volume = 1.0

    def setVolume(self, value):
        self.volume = value


class FakeSignal:
    def __init__(self):
        self.callbacks = []

    def connect(self, callback):
        self.callbacks.append(callback)

    def emit(self, *args):
        for callback in list(self.callbacks):
            callback(*args)


class FakeMediaPlayer:
    class PlaybackState:
        StoppedState = 0
        PlayingState = 1
        PausedState = 2

    def __init__(self):
        self.audio_output = None
        self.source = QUrl()
        self.position_ms = 0
        self.duration_ms = 0
        self.state = self.PlaybackState.StoppedState
        self.positionChanged = FakeSignal()
        self.durationChanged = FakeSignal()
        self.playbackStateChanged = FakeSignal()
        self.errorOccurred = FakeSignal()
        self.play_calls = 0
        self.pause_calls = 0
        self.stop_calls = 0

    def setAudioOutput(self, output):
        self.audio_output = output

    def setSource(self, source):
        self.source = source

    def play(self):
        self.play_calls += 1
        self.state = self.PlaybackState.PlayingState
        self.playbackStateChanged.emit(self.state)

    def pause(self):
        self.pause_calls += 1
        self.state = self.PlaybackState.PausedState
        self.playbackStateChanged.emit(self.state)

    def stop(self):
        self.stop_calls += 1
        self.state = self.PlaybackState.StoppedState
        self.playbackStateChanged.emit(self.state)

    def setPosition(self, value):
        self.position_ms = value
        self.positionChanged.emit(value)


class PlaybackTransportTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication([])

    def test_load_source_sets_url_and_duration(self):
        player = FakeMediaPlayer()
        audio = FakeAudioOutput()
        transport = PlaybackTransport(player=player, audio_output=audio)

        self.assertTrue(transport.load_source("/tmp/song.wav", 12.5))

        self.assertEqual(player.source.toLocalFile(), "/tmp/song.wav")
        self.assertEqual(transport.sourcePath, "/tmp/song.wav")
        self.assertEqual(transport.durationSeconds, 12.5)
        self.assertFalse(transport.isPlaying)

    def test_play_pause_stop_update_playing_state(self):
        player = FakeMediaPlayer()
        transport = PlaybackTransport(player=player, audio_output=FakeAudioOutput())
        transport.load_source("/tmp/song.wav", 10.0)

        transport.play()
        self.assertTrue(transport.isPlaying)
        self.assertEqual(player.play_calls, 1)

        transport.pause()
        self.assertFalse(transport.isPlaying)
        self.assertEqual(player.pause_calls, 1)

        transport.stop()
        self.assertFalse(transport.isPlaying)
        self.assertEqual(player.stop_calls, 1)
        self.assertEqual(transport.positionSeconds, 0.0)

    def test_seek_clamps_to_duration(self):
        player = FakeMediaPlayer()
        transport = PlaybackTransport(player=player, audio_output=FakeAudioOutput())
        transport.load_source("/tmp/song.wav", 8.0)

        transport.seek_seconds(12.0)

        self.assertEqual(player.position_ms, 8000)
        self.assertEqual(transport.positionSeconds, 8.0)

    def test_unload_clears_source_and_position(self):
        player = FakeMediaPlayer()
        transport = PlaybackTransport(player=player, audio_output=FakeAudioOutput())
        transport.load_source("/tmp/song.wav", 8.0)

        transport.unload()

        self.assertEqual(transport.sourcePath, "")
        self.assertEqual(transport.durationSeconds, 0.0)
        self.assertEqual(transport.positionSeconds, 0.0)
        self.assertFalse(transport.isPlaying)


if __name__ == "__main__":
    unittest.main()
```

- [x] **Step 2: Run playback transport tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_playback_transport -v
```

Expected: FAIL with `ModuleNotFoundError: No module named 'autolight.playback'`.

- [x] **Step 3: Implement the playback package export**

Create `autolight/playback/__init__.py`:

```python
from autolight.playback.transport import PlaybackTransport

__all__ = ["PlaybackTransport"]
```

- [x] **Step 4: Implement `PlaybackTransport`**

Create `autolight/playback/transport.py`:

```python
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

    @Slot(str, float, result=bool)
    def load_source(self, path: str, duration_seconds: float = 0.0) -> bool:
        source_path = str(Path(path))
        if not Path(source_path).is_file():
            self._set_last_error(f"audio file not found: {source_path}")
            return False
        self.stop()
        self._player.setSource(QUrl.fromLocalFile(source_path))
        self._set_source_path(source_path)
        self._set_duration_seconds(self._finite_non_negative(duration_seconds))
        self._set_position_seconds(0.0)
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
        self._audio_output.setVolume(min(max(self._finite_non_negative(value), 0.0), 1.0))

    def _handle_position_changed(self, milliseconds: int) -> None:
        self._set_position_seconds(self._finite_non_negative(milliseconds / 1000.0))

    def _handle_duration_changed(self, milliseconds: int) -> None:
        if self._duration_seconds <= 0.0:
            self._set_duration_seconds(self._finite_non_negative(milliseconds / 1000.0))

    def _handle_playback_state_changed(self, state) -> None:
        playback_state = getattr(self._player, "PlaybackState", QMediaPlayer.PlaybackState)
        playing_state = getattr(playback_state, "PlayingState", QMediaPlayer.PlaybackState.PlayingState)
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
```

- [x] **Step 5: Run playback transport tests**

Run:

```bash
uv run python -m unittest tests.test_playback_transport -v
```

Expected: PASS all tests in `tests.test_playback_transport`.

- [x] **Step 6: Commit playback transport**

```bash
git add autolight/playback/__init__.py autolight/playback/transport.py tests/test_playback_transport.py
git commit -m "Add playback transport wrapper"
```

## Task 2: Controller Playback And Viewport State

**Files:**
- Modify: `autolight/app_controller.py`
- Modify: `tests/test_app_controller.py`

- [x] **Step 1: Add failing controller playback tests**

Add this import to `tests/test_app_controller.py`:

```python
from unittest.mock import Mock
```

Add these tests to `AppControllerTest`:

```python
    def test_selected_source_track_can_play(self):
        controller = self._controller()

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path, frames=16000)
            track_id = controller.import_audio(str(audio_path))

            self.assertEqual(controller.selectedTrackId, track_id)
            self.assertTrue(controller.selectedTrackCanPlay)
            self.assertAlmostEqual(controller.timelineDurationSeconds, 2.0, places=2)

    def test_play_selected_track_loads_resolved_source_audio(self):
        controller = self._controller()
        controller.playback.load_source = Mock(return_value=True)
        controller.playback.play = Mock()

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path, frames=12000)
            controller.import_audio(str(audio_path))

            self.assertTrue(controller.play_selected_track())

        controller.playback.load_source.assert_called_once()
        loaded_path, loaded_duration = controller.playback.load_source.call_args.args
        self.assertEqual(loaded_path, str(audio_path))
        self.assertAlmostEqual(loaded_duration, 1.5, places=2)
        controller.playback.play.assert_called_once()

    def test_play_selected_track_rejects_track_without_source_audio(self):
        controller = self._controller()
        controller.load_demo_project()
        editable_id = controller._project.tracks[-1].id
        controller._project.audio_assets.clear()
        controller.select_track(editable_id)

        self.assertFalse(controller.play_selected_track())

        self.assertIn("source audio", controller.lastError)

    def test_timeline_zoom_and_scroll_are_clamped(self):
        controller = self._controller()
        self.assertEqual(controller.timelinePixelsPerSecond, 96.0)

        controller.set_timeline_zoom(500.0)
        self.assertEqual(controller.timelinePixelsPerSecond, 240.0)

        controller.set_timeline_zoom(5.0)
        self.assertEqual(controller.timelinePixelsPerSecond, 24.0)

        controller.set_timeline_scroll_seconds(-10.0)
        self.assertEqual(controller.timelineScrollSeconds, 0.0)
```

- [x] **Step 2: Run controller tests and verify failure**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_selected_source_track_can_play tests.test_app_controller.AppControllerTest.test_play_selected_track_loads_resolved_source_audio tests.test_app_controller.AppControllerTest.test_play_selected_track_rejects_track_without_source_audio tests.test_app_controller.AppControllerTest.test_timeline_zoom_and_scroll_are_clamped -v
```

Expected: FAIL with `AttributeError` for missing `playback`, `selectedTrackCanPlay`, `play_selected_track`, `timelineDurationSeconds`, `timelinePixelsPerSecond`, or timeline setter slots.

- [x] **Step 3: Import playback transport and add controller signals**

Update imports in `autolight/app_controller.py`:

```python
from autolight.playback import PlaybackTransport
from autolight.project.models import AudioAsset, Marker, ResultState, TrackType
```

Add these signals to `AppController`:

```python
    selectedTrackCanPlayChanged = Signal()
    timelineDurationSecondsChanged = Signal()
    timelinePixelsPerSecondChanged = Signal()
    timelineScrollSecondsChanged = Signal()
```

- [x] **Step 4: Initialize playback and viewport state**

Add these fields in `AppController.__init__` after `_runtime_temp_dir` is created:

```python
        self._playback = PlaybackTransport(parent=self)
        self._timeline_pixels_per_second = 96.0
        self._timeline_scroll_seconds = 0.0
```

- [x] **Step 5: Expose playback and timeline properties**

Add these properties below `transformModel`:

```python
    @Property(QObject, constant=True)
    def playback(self):
        return self._playback

    @Property(bool, notify=selectedTrackCanPlayChanged)
    def selectedTrackCanPlay(self) -> bool:
        asset = self._source_audio_asset_for_track_id(self._selected_track_id)
        return asset is not None and asset.import_status == "online"

    @Property(float, notify=timelineDurationSecondsChanged)
    def timelineDurationSeconds(self) -> float:
        return self._timeline_duration_seconds()

    @Property(float, notify=timelinePixelsPerSecondChanged)
    def timelinePixelsPerSecond(self) -> float:
        return self._timeline_pixels_per_second

    @Property(float, notify=timelineScrollSecondsChanged)
    def timelineScrollSeconds(self) -> float:
        return self._timeline_scroll_seconds
```

- [x] **Step 6: Add playback and viewport slots**

Add these slots above `cleanup`:

```python
    @Slot(result=bool)
    def play_selected_track(self) -> bool:
        asset = self._source_audio_asset_for_track_id(self._selected_track_id)
        if asset is None:
            self._set_last_error("selected track has no source audio")
            return False
        if asset.import_status != "online":
            self._set_last_error(f"source audio is {asset.import_status}")
            return False
        if not self._playback.load_source(asset.path, asset.duration):
            self._set_last_error(self._playback.lastError)
            return False
        self._playback.play()
        self._set_last_error("")
        return True

    @Slot()
    def pause_playback(self) -> None:
        self._playback.pause()

    @Slot()
    def stop_playback(self) -> None:
        self._playback.stop()

    @Slot(float)
    def seek_playback(self, seconds: float) -> None:
        self._playback.seek_seconds(seconds)
        self.set_timeline_scroll_seconds(self._scroll_for_visible_time(seconds))

    @Slot(float)
    def set_timeline_zoom(self, pixels_per_second: float) -> None:
        clamped = min(max(float(pixels_per_second), 24.0), 240.0)
        if self._timeline_pixels_per_second == clamped:
            return
        self._timeline_pixels_per_second = clamped
        self.timelinePixelsPerSecondChanged.emit()
        self.set_timeline_scroll_seconds(self._timeline_scroll_seconds)

    @Slot(float)
    def set_timeline_scroll_seconds(self, seconds: float) -> None:
        duration = self._timeline_duration_seconds()
        visible_seconds = self._visible_timeline_seconds()
        maximum = max(0.0, duration - visible_seconds)
        clamped = min(max(float(seconds), 0.0), maximum)
        if self._timeline_scroll_seconds == clamped:
            return
        self._timeline_scroll_seconds = clamped
        self.timelineScrollSecondsChanged.emit()
```

- [x] **Step 7: Add source-audio and duration helpers**

Add these helpers near `_source_audio_path_for_track`:

```python
    def _source_audio_asset_for_track_id(self, track_id: str) -> AudioAsset | None:
        track = find_track(self._project, track_id)
        if track is None:
            return None
        seen_track_ids = set()
        pending = [track]
        while pending:
            current = pending.pop(0)
            if current is None or current.id in seen_track_ids:
                continue
            seen_track_ids.add(current.id)
            asset_id = current.provenance.get("asset_id")
            asset = next((item for item in self._project.audio_assets if item.id == asset_id), None)
            if asset is not None:
                return asset
            next_track_ids = list(current.input_track_ids)
            source_track_id = current.provenance.get("source_track_id", "")
            if source_track_id:
                next_track_ids.append(source_track_id)
            for next_track_id in next_track_ids:
                candidate = find_track(self._project, next_track_id)
                if candidate is not None:
                    pending.append(candidate)
        return None

    def _timeline_duration_seconds(self) -> float:
        audio_duration = max((asset.duration for asset in self._project.audio_assets), default=0.0)
        marker_duration = max(
            (
                marker.timestamp + (marker.duration or 0.0)
                for marker in self._project.markers
            ),
            default=0.0,
        )
        return max(8.0, audio_duration, marker_duration, self._playback.durationSeconds)

    def _visible_timeline_seconds(self) -> float:
        return 8.0

    def _scroll_for_visible_time(self, seconds: float) -> float:
        duration = self._timeline_duration_seconds()
        visible_seconds = self._visible_timeline_seconds()
        if seconds < self._timeline_scroll_seconds:
            return seconds
        if seconds > self._timeline_scroll_seconds + visible_seconds:
            return seconds - visible_seconds
        return self._timeline_scroll_seconds
```

- [x] **Step 8: Emit playback-related changes when project or selection changes**

Update `_set_project`:

```python
    def _set_project(self, project) -> None:
        self._playback.unload()
        self._project = project
        self._load_all_waveform_samples()
        self._track_model.set_project(self._project)
        self._timeline_scroll_seconds = 0.0
        self.projectNameChanged.emit()
        self.selectedTrackCanRerunChanged.emit()
        self.selectedTrackCanPlayChanged.emit()
        self.timelineDurationSecondsChanged.emit()
        self.timelineScrollSecondsChanged.emit()
```

Update `_set_selected_track_id`:

```python
    def _set_selected_track_id(self, track_id: str) -> None:
        if self._selected_track_id == track_id:
            return
        self._selected_track_id = track_id
        self.selectedTrackIdChanged.emit()
        self.selectedTrackMarkersChanged.emit()
        self.selectedTrackHasRunningJobChanged.emit()
        self.selectedTrackCanRerunChanged.emit()
        self.selectedTrackCanPlayChanged.emit()
```

Update `import_audio`, `add_fixed_interval_track`, `add_transform_track`, `create_editable_track_from_track`, and `_handle_track_changed` to emit `timelineDurationSecondsChanged` after the project content changes:

```python
            self.timelineDurationSecondsChanged.emit()
```

- [x] **Step 9: Run controller playback tests**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_selected_source_track_can_play tests.test_app_controller.AppControllerTest.test_play_selected_track_loads_resolved_source_audio tests.test_app_controller.AppControllerTest.test_play_selected_track_rejects_track_without_source_audio tests.test_app_controller.AppControllerTest.test_timeline_zoom_and_scroll_are_clamped -v
```

Expected: PASS for the four controller playback and viewport tests.

- [x] **Step 10: Commit controller playback and viewport state**

```bash
git add autolight/app_controller.py tests/test_app_controller.py
git commit -m "Expose playback and timeline viewport state"
```

## Task 3: QML Transport Controls And Playhead

**Files:**
- Modify: `UI/Main.qml`
- Modify: `tests/test_app_controller.py`

- [x] **Step 1: Add failing QML transport wiring test**

Add this test to `AppControllerTest`:

```python
    def test_qml_exposes_transport_controls_and_playhead(self):
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")

        self.assertIn("appController.play_selected_track()", qml)
        self.assertIn("appController.pause_playback()", qml)
        self.assertIn("appController.stop_playback()", qml)
        self.assertIn("appController.seek_playback", qml)
        self.assertIn("appController.playback.positionSeconds", qml)
        self.assertIn("appController.playback.durationSeconds", qml)
        self.assertIn("id: playhead", qml)
        self.assertIn("playheadTimeLabel", qml)
```

- [x] **Step 2: Run QML transport test and verify failure**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_qml_exposes_transport_controls_and_playhead -v
```

Expected: FAIL because `UI/Main.qml` does not yet reference playback controls or a playhead.

- [x] **Step 3: Add formatting helpers and playback position binding**

In `UI/Main.qml`, add these root properties and helper functions after the existing default marker properties:

```qml
    property real timelineVisibleSeconds: 8.0

    function timelineX(seconds) {
        return root.timelineLeftPadding + (seconds - appController.timelineScrollSeconds) * appController.timelinePixelsPerSecond
    }

    function formatSeconds(seconds) {
        var safeSeconds = Math.max(0, Number(seconds))
        var minutes = Math.floor(safeSeconds / 60)
        var remaining = Math.floor(safeSeconds % 60)
        return minutes + ":" + (remaining < 10 ? "0" + remaining : remaining)
    }
```

- [x] **Step 4: Add toolbar transport controls**

Add these controls in the toolbar `RowLayout` before `New`:

```qml
                Button {
                    text: appController.playback.isPlaying ? "Pause" : "Play"
                    enabled: appController.selectedTrackCanPlay || appController.playback.sourcePath.length > 0
                    onClicked: appController.playback.isPlaying ? appController.pause_playback() : appController.play_selected_track()
                }

                Button {
                    text: "Stop"
                    enabled: appController.playback.sourcePath.length > 0
                    onClicked: appController.stop_playback()
                }

                Label {
                    id: playheadTimeLabel
                    text: root.formatSeconds(appController.playback.positionSeconds) + " / " + root.formatSeconds(appController.playback.durationSeconds)
                    color: "#d4d4d8"
                    font.pixelSize: 12
                }
```

- [x] **Step 5: Add scrubber under the ruler**

Add this `Slider` after the `timelineRuler` `RowLayout`:

```qml
        Slider {
            id: playbackScrubber
            Layout.fillWidth: true
            from: 0
            to: Math.max(0.01, appController.playback.durationSeconds)
            value: appController.playback.positionSeconds
            enabled: appController.playback.sourcePath.length > 0
            live: true
            onMoved: appController.seek_playback(value)
        }
```

- [x] **Step 6: Add a playhead overlay to each timeline lane**

Inside the timeline lane `Rectangle` that contains waveform and marker repeaters, add this `Rectangle` before the lane `MouseArea`:

```qml
                        Rectangle {
                            id: playhead
                            width: 2
                            height: parent.height
                            x: root.timelineX(appController.playback.positionSeconds)
                            color: "#facc15"
                            visible: appController.playback.sourcePath.length > 0
                                && x >= root.timelineLeftPadding
                                && x <= parent.width
                            z: 10
                        }
```

- [x] **Step 7: Use `timelineX` for marker positioning**

Replace the marker x binding:

```qml
                                x: Math.max(0, Math.min(parent.width - width, root.timelineLeftPadding + modelData.timestamp * root.timelinePixelsPerSecond))
```

with:

```qml
                                x: Math.max(0, Math.min(parent.width - width, root.timelineX(modelData.timestamp)))
```

- [x] **Step 8: Run QML transport test and smoke check**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_qml_exposes_transport_controls_and_playhead -v
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: PASS for the QML wiring test and exit 0 for the smoke check.

- [x] **Step 9: Commit QML transport controls**

```bash
git add UI/Main.qml tests/test_app_controller.py
git commit -m "Add playback controls and playhead"
```

## Task 4: Timeline Zoom And Horizontal Navigation

**Files:**
- Modify: `UI/Main.qml`
- Modify: `tests/test_app_controller.py`

- [x] **Step 1: Add failing QML zoom and pan wiring test**

Add this test to `AppControllerTest`:

```python
    def test_qml_exposes_timeline_zoom_and_scroll_controls(self):
        qml = (Path(__file__).resolve().parents[1] / "UI" / "Main.qml").read_text(encoding="utf-8")

        self.assertIn("id: timelineZoomSlider", qml)
        self.assertIn("appController.set_timeline_zoom", qml)
        self.assertIn("id: timelineScrollSlider", qml)
        self.assertIn("appController.set_timeline_scroll_seconds", qml)
        self.assertIn("appController.timelinePixelsPerSecond", qml)
        self.assertIn("appController.timelineScrollSeconds", qml)
        self.assertIn("appController.timelineDurationSeconds", qml)
        self.assertNotIn("readonly property real timelinePixelsPerSecond: 96", qml)
```

- [x] **Step 2: Run QML zoom test and verify failure**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_qml_exposes_timeline_zoom_and_scroll_controls -v
```

Expected: FAIL because the timeline still uses a fixed root `timelinePixelsPerSecond` constant and has no horizontal navigation controls.

- [x] **Step 3: Remove the fixed root zoom constant**

In `UI/Main.qml`, remove this root property:

```qml
    readonly property real timelinePixelsPerSecond: 96
```

Update all `root.timelinePixelsPerSecond` references to `appController.timelinePixelsPerSecond`.

- [x] **Step 4: Add zoom and scroll controls**

Add this row below the playback scrubber:

```qml
        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 12
            Layout.rightMargin: 12
            spacing: 10

            Label {
                text: "Zoom"
                color: "#d4d4d8"
                font.pixelSize: 12
            }

            Slider {
                id: timelineZoomSlider
                from: 24
                to: 240
                value: appController.timelinePixelsPerSecond
                Layout.preferredWidth: 180
                onMoved: appController.set_timeline_zoom(value)
            }

            Label {
                text: Math.round(appController.timelinePixelsPerSecond) + " px/s"
                color: "#a1a1aa"
                font.pixelSize: 12
                Layout.preferredWidth: 64
            }

            Slider {
                id: timelineScrollSlider
                from: 0
                to: Math.max(0, appController.timelineDurationSeconds - root.timelineVisibleSeconds)
                value: appController.timelineScrollSeconds
                Layout.fillWidth: true
                onMoved: appController.set_timeline_scroll_seconds(value)
            }
        }
```

- [x] **Step 5: Make ruler ticks respect zoom and scroll**

Replace the ruler `Row` inside `timelineRuler` with this `Repeater`:

```qml
                Repeater {
                    model: Math.ceil(root.timelineVisibleSeconds) + 1
                    Text {
                        x: root.timelineX(appController.timelineScrollSeconds + index)
                        y: 9
                        text: Math.floor(appController.timelineScrollSeconds + index) + "s"
                        color: "#a1a1aa"
                        font.pixelSize: 12
                    }
                }
```

- [x] **Step 6: Make waveform rendering respect horizontal scroll**

Replace the waveform `x` binding:

```qml
                                x: root.timelineLeftPadding + (waveformSamples.length > 1 ? index * Math.max(0, parent.width - root.timelineLeftPadding - width) / (waveformSamples.length - 1) : 0)
```

with:

```qml
                                x: root.timelineX(index / Math.max(1, waveformSamples.length - 1) * appController.timelineDurationSeconds)
                                visible: x >= root.timelineLeftPadding - width && x <= parent.width
```

- [x] **Step 7: Update the old QML timeline test expectation**

In `test_qml_timeline_shell_uses_one_row_oriented_list`, replace:

```python
        self.assertIn("spacing: root.timelinePixelsPerSecond", qml)
        self.assertIn("modelData.timestamp * root.timelinePixelsPerSecond", qml)
```

with:

```python
        self.assertIn("root.timelineX(modelData.timestamp)", qml)
        self.assertIn("appController.timelinePixelsPerSecond", qml)
```

- [x] **Step 8: Run zoom tests and smoke**

Run:

```bash
uv run python -m unittest tests.test_app_controller.AppControllerTest.test_qml_exposes_timeline_zoom_and_scroll_controls tests.test_app_controller.AppControllerTest.test_qml_timeline_shell_uses_one_row_oriented_list -v
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: PASS for both QML tests and exit 0 for the smoke check.

- [x] **Step 9: Commit zoom and navigation controls**

```bash
git add UI/Main.qml tests/test_app_controller.py
git commit -m "Add timeline zoom and horizontal navigation"
```

## Task 5: Documentation And Final Verification

**Files:**
- Modify: `README.md`
- Modify: `docs/superpowers/plans/2026-06-01-autolight-playback-navigation.md`

- [x] **Step 1: Update README workflow**

In `README.md`, replace the current `Basic Workflow` list with:

```markdown
## Basic Workflow

1. Launch the app with `uv run python main.py`.
2. Use `Import Audio` to add a local audio file as a source track.
3. Select the source track and use `Play`, `Pause`, `Stop`, or the scrubber to inspect the audio.
4. Use the timeline zoom and horizontal navigation controls to inspect markers at the needed time scale.
5. With the source track selected, choose `Add Markers` or `Add Transform` to create generated marker tracks.
6. Run generated tracks by selecting them and choosing `Run`.
7. After completion, choose `Derive Editable` to create editable cue markers from a generated track.
8. Use `Save` or `Save As` to write a `.autolight` project file.
9. Use `Open` to reload a saved project.
```

- [x] **Step 2: Run the full unit suite**

Run:

```bash
uv run python -m unittest discover -s tests -v
```

Expected: PASS all tests.

- [x] **Step 3: Run the headless smoke check**

Run:

```bash
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

Expected: exit 0. A Qt font alias warning is acceptable if no QML loading error appears.

- [x] **Step 4: Check whitespace and final status**

Run:

```bash
git diff --check
git status --short
```

Expected: `git diff --check` exits 0. `git status --short` shows only the intended playback/navigation files before the final commit.

- [x] **Step 5: Mark this implementation plan complete**

After all previous steps pass, update every unchecked box in `docs/superpowers/plans/2026-06-01-autolight-playback-navigation.md` to checked for completed work.

- [x] **Step 6: Commit documentation and plan completion**

```bash
git add README.md docs/superpowers/plans/2026-06-01-autolight-playback-navigation.md
git commit -m "Document playback navigation workflow"
```

## Execution Notes

- Keep playback state out of `.autolight` project files for this milestone. Project files should persist source audio metadata and timeline content, not volatile transport position.
- Do not add waveform generation to this plan. Existing waveform summary work remains a generated transform; this milestone only makes the displayed timeline navigable.
- Use the existing text-button QML style for consistency with the current shell. A later visual polish pass can replace toolbar text with icons.
- Stop or unload playback when replacing projects so a stale media source cannot keep playing after `New`, `Open`, or `Load Demo`.
