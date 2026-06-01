import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import numpy as np
import soundfile

from autolight.analysis.builtin import register_builtin_transforms
from autolight.analysis.registry import TransformCancelled, TransformContext, TransformRegistry
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
        self.assertTrue(all(marker["confidence"] is None for marker in markers))

    def test_detect_beat_markers_returns_timing_markers(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "clicks.wav"
            write_click_track(audio_path)

            markers = detect_beat_markers(audio_path)

        self.assertGreaterEqual(len(markers), 1)
        self.assertTrue(all(marker["label"] == "Beat" for marker in markers))
        self.assertTrue(all(marker["confidence"] is None for marker in markers))

    def test_detect_beat_markers_handles_scalar_numpy_tempo(self):
        with (
            patch("autolight.analysis.timing._load_audio", return_value=(np.zeros(16), 8)),
            patch(
                "autolight.analysis.timing.librosa.beat.beat_track",
                return_value=(np.asarray(120.0), np.asarray([4])),
            ),
            patch("autolight.analysis.timing.librosa.frames_to_time", return_value=np.asarray([0.5])),
        ):
            markers = detect_beat_markers("song.wav")

        self.assertEqual(markers[0]["metadata"]["tempo"], 120.0)

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

    def test_timing_transforms_cancel_before_loading_audio(self):
        registry = TransformRegistry()
        register_builtin_transforms(registry)
        context = TransformContext(
            artifact_dir=Path("artifacts"),
            cancel_requested=lambda: True,
            progress=lambda value: None,
        )

        with self.assertRaises(TransformCancelled):
            registry.get("timing.onsets", version="1").run(context, {"audio_path": "missing.wav"})
        with self.assertRaises(TransformCancelled):
            registry.get("timing.beats", version="1").run(context, {"audio_path": "missing.wav"})


if __name__ == "__main__":
    unittest.main()
