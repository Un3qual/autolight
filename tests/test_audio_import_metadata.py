import math
import tempfile
import unittest
import wave
from pathlib import Path

from autolight.project.audio_probe import probe_audio_file
from autolight.project.store import import_audio_asset, new_project


def write_wav(path: Path, *, sample_rate: int = 8000, channels: int = 1, frames: int = 8000) -> None:
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(channels)
        handle.setsampwidth(2)
        handle.setframerate(sample_rate)
        handle.writeframes(b"\0\0" * frames * channels)


class AudioImportMetadataTest(unittest.TestCase):
    def test_probe_audio_file_returns_duration_sample_rate_and_channels(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path, sample_rate=8000, channels=2, frames=4000)

            metadata = probe_audio_file(audio_path)

        self.assertTrue(math.isclose(metadata.duration, 0.5, rel_tol=0.01))
        self.assertEqual(metadata.sample_rate, 8000)
        self.assertEqual(metadata.channels, 2)

    def test_probe_audio_file_rejects_directory(self):
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(IsADirectoryError, "not a file"):
                probe_audio_file(Path(tmp))

    def test_import_audio_asset_populates_real_metadata(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path, sample_rate=11025, channels=1, frames=22050)
            project = new_project("Demo")

            track = import_audio_asset(project, audio_path)

        self.assertEqual(track.name, "song")
        self.assertEqual(len(project.audio_assets), 1)
        asset = project.audio_assets[0]
        self.assertTrue(math.isclose(asset.duration, 2.0, rel_tol=0.01))
        self.assertEqual(asset.sample_rate, 11025)
        self.assertEqual(asset.channels, 1)
        self.assertEqual(asset.import_status, "online")
        self.assertEqual(asset.relink_hint, "")
        self.assertNotEqual(asset.fingerprint, "")


if __name__ == "__main__":
    unittest.main()
