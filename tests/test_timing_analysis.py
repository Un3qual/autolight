import tempfile
import unittest
from pathlib import Path

import numpy as np
import soundfile

from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformContext, TransformRegistry
from autolight.analysis.timing import detect_beat_markers, detect_onset_markers


def write_click_track(path: Path, sample_rate: int = 22050) -> None:
    audio = np.zeros(sample_rate * 2, dtype=np.float32)
    audio[0] = 1.0
    audio[sample_rate // 2] = 1.0
    audio[sample_rate] = 1.0
    audio[(sample_rate * 3) // 2] = 1.0
    soundfile.write(str(path), audio, sample_rate)


class TimingAnalysisTest(unittest.TestCase):
    def test_detect_onset_markers_returns_timestamped_markers(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "clicks.wav"
            write_click_track(audio_path)

            markers = detect_onset_markers(audio_path)

        self.assertGreaterEqual(len(markers), 2)
        self.assertTrue(all(marker["category"] == "onset" for marker in markers))
        self.assertTrue(all(marker["timestamp"] >= 0 for marker in markers))

    def test_detect_beat_markers_returns_timing_markers(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "clicks.wav"
            write_click_track(audio_path)

            markers = detect_beat_markers(audio_path)

        self.assertGreaterEqual(len(markers), 1)
        self.assertTrue(all(marker["label"] == "Beat" for marker in markers))

    def test_timing_transforms_are_registered_and_return_markers(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)

        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "clicks.wav"
            write_click_track(audio_path)
            context = TransformContext(
                artifact_dir=Path(tmp) / "artifacts",
                cancel_requested=lambda: False,
                progress=lambda value: None,
            )

            onset_result = registry.get("timing.onsets", version="1").run(
                context, {"audio_path": str(audio_path)}
            )
            beat_result = registry.get("timing.beats", version="1").run(
                context, {"audio_path": str(audio_path)}
            )

        self.assertGreaterEqual(len(onset_result.markers), 2)
        self.assertGreaterEqual(len(beat_result.markers), 1)


if __name__ == "__main__":
    unittest.main()
