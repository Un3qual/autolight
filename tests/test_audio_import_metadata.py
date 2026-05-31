import math
import tempfile
import unittest
import wave
from pathlib import Path

from autolight.project.audio_probe import probe_audio_file


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


if __name__ == "__main__":
    unittest.main()
