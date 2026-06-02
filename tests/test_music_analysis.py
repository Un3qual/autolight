import json
import tempfile
import unittest
import wave
from pathlib import Path

from autolight.app.analysis_lod import AnalysisLodStore
from autolight.analysis.music import MusicAnalysisEngine


def write_impulse_wav(path: Path, *, sample_rate: int = 8000, seconds: float = 2.0) -> None:
    frame_count = int(sample_rate * seconds)
    samples = []
    for index in range(frame_count):
        value = 20000 if index % (sample_rate // 2) == 0 else 0
        samples.append(value.to_bytes(2, "little", signed=True))
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(sample_rate)
        handle.writeframes(b"".join(samples))


class AnalysisLodStoreTest(unittest.TestCase):
    def test_visible_frames_returns_bounded_time_window(self):
        payload = {
            "version": 1,
            "kind": "energy",
            "duration": 10.0,
            "frames": [{"time": float(index), "intensity": index / 10.0} for index in range(10)],
        }
        visible = AnalysisLodStore().visible_frames(
            payload,
            scroll_seconds=2.0,
            visible_seconds=3.0,
            max_frames=4,
        )

        self.assertEqual([frame["time"] for frame in visible["frames"]], [2.0, 3.0, 4.0, 5.0])
        self.assertEqual(visible["kind"], "energy")


class MusicAnalysisEngineTest(unittest.TestCase):
    def test_energy_profile_returns_bounded_normalized_frames(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "impulses.wav"
            write_impulse_wav(audio_path)
            result = MusicAnalysisEngine().analyze_energy(audio_path, {"max_frames": 32})

        self.assertEqual(result.kind, "energy")
        self.assertLessEqual(len(result.frames), 32)
        self.assertTrue(all(0.0 <= frame["intensity"] <= 1.0 for frame in result.frames))
        self.assertTrue(any(marker["category"] == "energy_peak" for marker in result.markers))

    def test_energy_profile_honors_max_markers(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "impulses.wav"
            write_impulse_wav(audio_path)
            result = MusicAnalysisEngine().analyze_energy(audio_path, {"max_frames": 32, "max_markers": 1})

        self.assertLessEqual(len(result.markers), 1)
        self.assertEqual(result.payload["settings"]["max_markers"], 1)

    def test_harmony_profile_returns_chroma_color_frames(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "impulses.wav"
            write_impulse_wav(audio_path)
            result = MusicAnalysisEngine().analyze_harmony(audio_path, {"max_frames": 16})

        self.assertEqual(result.kind, "harmonic-color")
        self.assertLessEqual(len(result.frames), 16)
        if result.frames:
            self.assertEqual(len(result.frames[0]["chroma"]), 12)
            self.assertIn("color", result.frames[0])

    def test_harmony_profile_honors_max_markers(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "impulses.wav"
            write_impulse_wav(audio_path)
            result = MusicAnalysisEngine().analyze_harmony(audio_path, {"max_frames": 16, "max_markers": 1})

        self.assertLessEqual(len(result.markers), 1)
        self.assertEqual(result.payload["settings"]["max_markers"], 1)

    def test_beat_grid_returns_artifact_payload_and_marker_dicts(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "impulses.wav"
            write_impulse_wav(audio_path, seconds=4.0)
            result = MusicAnalysisEngine().analyze_rhythm(audio_path, {"max_markers": 64})

        self.assertEqual(result.kind, "beat-grid")
        self.assertIn("version", result.payload)
        self.assertLessEqual(len(result.markers), 64)
        self.assertTrue(all("timestamp" in marker for marker in result.markers))

    def test_invalid_numeric_settings_fail_before_audio_loading(self):
        missing_audio = Path("does-not-exist.wav")
        engine = MusicAnalysisEngine()

        with self.assertRaisesRegex(ValueError, "hop_length"):
            engine.analyze_rhythm(missing_audio, {"hop_length": 0})
        with self.assertRaisesRegex(ValueError, "max_frames"):
            engine.analyze_energy(missing_audio, {"max_frames": 0})
        with self.assertRaisesRegex(ValueError, "max_markers"):
            engine.analyze_harmony(missing_audio, {"max_markers": 0})

    def test_analysis_results_are_strict_json_serializable(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "impulses.wav"
            write_impulse_wav(audio_path, seconds=4.0)
            results = [
                MusicAnalysisEngine().analyze_energy(audio_path, {"max_frames": 32, "max_markers": 4}),
                MusicAnalysisEngine().analyze_harmony(audio_path, {"max_frames": 16, "max_markers": 4}),
                MusicAnalysisEngine().analyze_rhythm(audio_path, {"max_markers": 64}),
            ]

        for result in results:
            json.dumps(result.payload, allow_nan=False)
            json.dumps(result.markers, allow_nan=False)
