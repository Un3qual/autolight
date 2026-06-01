import json
import tempfile
import unittest
import wave
from pathlib import Path

from autolight.analysis.waveform import build_waveform_summary


def write_wav(path: Path) -> None:
    samples = [0, 1000, -1000, 2000, -2000, 0, 500, -500]
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(8)
        handle.writeframes(b"".join(sample.to_bytes(2, "little", signed=True) for sample in samples))


class WaveformSummaryTest(unittest.TestCase):
    def test_build_waveform_summary_returns_normalized_buckets(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            output_path = Path(tmp) / "waveform.json"
            write_wav(audio_path)

            build_waveform_summary(audio_path, output_path, buckets=4)
            payload = json.loads(output_path.read_text(encoding="utf-8"))

        self.assertEqual(payload["version"], 1)
        self.assertEqual(payload["sample_rate"], 8)
        self.assertEqual(len(payload["samples"]), 4)
        self.assertTrue(all(0.0 <= item["peak"] <= 1.0 for item in payload["samples"]))
        self.assertTrue(all(0.0 <= item["rms"] <= 1.0 for item in payload["samples"]))


if __name__ == "__main__":
    unittest.main()
