from pathlib import Path
import tempfile
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
        self.events = []
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
        self.events.append(("setSource", source.toLocalFile()))
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
        self.events.append(("stop", None))
        self.stop_calls += 1
        self.state = self.PlaybackState.StoppedState
        self.playbackStateChanged.emit(self.state)

    def setPosition(self, value):
        self.position_ms = value
        self.positionChanged.emit(value)


class PositionFirstStopMediaPlayer(FakeMediaPlayer):
    def stop(self):
        self.events.append(("stop", None))
        self.stop_calls += 1
        self.positionChanged.emit(0)
        self.state = self.PlaybackState.StoppedState
        self.playbackStateChanged.emit(self.state)


class PlaybackTransportTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QCoreApplication.instance() or QCoreApplication([])

    @staticmethod
    def _temp_audio_path(directory):
        path = Path(directory) / "song.wav"
        path.write_bytes(b"fake audio")
        return str(path)

    def test_load_source_sets_url_and_duration(self):
        player = FakeMediaPlayer()
        audio = FakeAudioOutput()
        transport = PlaybackTransport(player=player, audio_output=audio)

        with tempfile.TemporaryDirectory() as directory:
            path = self._temp_audio_path(directory)

            self.assertTrue(transport.load_source(path, 12.5))

        self.assertEqual(player.source.toLocalFile(), path)
        self.assertEqual(transport.sourcePath, path)
        self.assertEqual(transport.durationSeconds, 12.5)
        self.assertFalse(transport.isPlaying)

    def test_load_source_returns_false_for_missing_file(self):
        player = FakeMediaPlayer()
        transport = PlaybackTransport(player=player, audio_output=FakeAudioOutput())

        missing_path = "/tmp/autolight-missing-song.wav"

        self.assertFalse(transport.load_source(missing_path, 12.5))

        self.assertEqual(transport.lastError, f"audio file not found: {missing_path}")
        self.assertEqual(player.stop_calls, 0)
        self.assertEqual(player.source.toLocalFile(), "")
        self.assertEqual(transport.sourcePath, "")

    def test_missing_file_load_clears_existing_source_and_playback(self):
        player = FakeMediaPlayer()
        transport = PlaybackTransport(player=player, audio_output=FakeAudioOutput())

        with tempfile.TemporaryDirectory() as directory:
            current_path = self._temp_audio_path(directory)
            missing_path = str(Path(directory) / "removed.wav")

            self.assertTrue(transport.load_source(current_path, 12.5))
            transport.play()

            self.assertFalse(transport.load_source(missing_path, 9.0))

        self.assertEqual(transport.lastError, f"audio file not found: {missing_path}")
        self.assertEqual(player.source.toLocalFile(), "")
        self.assertEqual(transport.sourcePath, "")
        self.assertEqual(transport.durationSeconds, 0.0)
        self.assertEqual(transport.positionSeconds, 0.0)
        self.assertFalse(transport.isPlaying)
        self.assertEqual(player.stop_calls, 2)

    def test_load_source_stops_playback_before_setting_new_source(self):
        player = FakeMediaPlayer()
        transport = PlaybackTransport(player=player, audio_output=FakeAudioOutput())

        with tempfile.TemporaryDirectory() as directory:
            path = self._temp_audio_path(directory)

            self.assertTrue(transport.load_source(path, 12.5))

        self.assertEqual(player.events[:2], [("stop", None), ("setSource", path)])

    def test_play_pause_stop_update_playing_state(self):
        player = FakeMediaPlayer()
        transport = PlaybackTransport(player=player, audio_output=FakeAudioOutput())
        with tempfile.TemporaryDirectory() as directory:
            transport.load_source(self._temp_audio_path(directory), 10.0)
            player.stop_calls = 0

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

    def test_stop_marks_not_playing_before_position_reset_signal(self):
        player = PositionFirstStopMediaPlayer()
        transport = PlaybackTransport(player=player, audio_output=FakeAudioOutput())
        with tempfile.TemporaryDirectory() as directory:
            transport.load_source(self._temp_audio_path(directory), 8.0)
            transport.play()
            transport.seek_seconds(2.0)
            playing_states_on_position_change = []
            transport.positionSecondsChanged.connect(
                lambda: playing_states_on_position_change.append(transport.isPlaying)
            )

            transport.stop()

        self.assertEqual(playing_states_on_position_change, [False])
        self.assertFalse(transport.isPlaying)
        self.assertEqual(transport.positionSeconds, 0.0)

    def test_seek_clamps_to_duration(self):
        player = FakeMediaPlayer()
        transport = PlaybackTransport(player=player, audio_output=FakeAudioOutput())
        with tempfile.TemporaryDirectory() as directory:
            transport.load_source(self._temp_audio_path(directory), 8.0)

            transport.seek_seconds(12.0)

            self.assertEqual(player.position_ms, 8000)
            self.assertEqual(transport.positionSeconds, 8.0)

    def test_backend_duration_can_extend_undersized_hint_for_seek_clamping(self):
        player = FakeMediaPlayer()
        transport = PlaybackTransport(player=player, audio_output=FakeAudioOutput())
        with tempfile.TemporaryDirectory() as directory:
            transport.load_source(self._temp_audio_path(directory), 8.0)

            player.durationChanged.emit(12_000)
            transport.seek_seconds(11.0)

            self.assertEqual(transport.durationSeconds, 12.0)
            self.assertEqual(player.position_ms, 11_000)
            self.assertEqual(transport.positionSeconds, 11.0)

    def test_unload_clears_source_and_position(self):
        player = FakeMediaPlayer()
        transport = PlaybackTransport(player=player, audio_output=FakeAudioOutput())
        with tempfile.TemporaryDirectory() as directory:
            transport.load_source(self._temp_audio_path(directory), 8.0)

            transport.unload()

            self.assertEqual(transport.sourcePath, "")
            self.assertEqual(transport.durationSeconds, 0.0)
            self.assertEqual(transport.positionSeconds, 0.0)
            self.assertFalse(transport.isPlaying)


if __name__ == "__main__":
    unittest.main()
