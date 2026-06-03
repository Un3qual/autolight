import json
import tempfile
import unittest
import wave
from pathlib import Path
from unittest.mock import patch

import numpy as np

from autolight.app.analysis_lod import AnalysisLodStore
from autolight.analysis.music import (
    MusicAnalysisCancelled,
    MusicAnalysisEngine,
    _chroma_frames,
    _harmonic_change_markers,
)


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

    def test_visible_frames_excludes_malformed_frame_times(self):
        payload = {
            "version": 1,
            "kind": "energy",
            "duration": 2.0,
            "frames": [
                {"id": "missing"},
                {"id": "malformed", "time": "not-a-time"},
                {"id": "nan", "time": float("nan")},
                {"id": "infinite", "time": float("inf")},
                {"id": "valid-zero", "time": 0.0},
                {"id": "valid-coerced", "time": "1.0"},
            ],
        }

        visible = AnalysisLodStore().visible_frames(
            payload,
            scroll_seconds=0.0,
            visible_seconds=2.0,
        )

        self.assertEqual([frame["id"] for frame in visible["frames"]], ["valid-zero", "valid-coerced"])

    def test_visible_harmonic_frames_preserve_left_edge_color_context(self):
        payload = {
            "version": 1,
            "kind": "harmonic-color",
            "duration": 10.0,
            "frames": [
                {"time": 2.0, "color": "#f00"},
                {"time": 3.0, "color": "#0f0"},
                {"time": 4.0, "color": "#00f"},
            ],
        }

        visible = AnalysisLodStore().visible_frames(
            payload,
            scroll_seconds=2.3,
            visible_seconds=1.5,
        )

        self.assertEqual([frame["time"] for frame in visible["frames"]], [2.3, 3.0])
        self.assertEqual(visible["frames"][0]["color"], "#f00")

    def test_visible_frames_coerces_missing_kind_to_empty_string(self):
        visible = AnalysisLodStore().visible_frames(
            {"kind": None, "frames": []},
            scroll_seconds=0.0,
            visible_seconds=1.0,
        )

        self.assertEqual(visible["kind"], "")

    def test_visible_frames_rejects_non_positive_max_frames(self):
        with self.assertRaisesRegex(ValueError, "max_frames"):
            AnalysisLodStore().visible_frames(
                {"kind": "energy", "frames": []},
                scroll_seconds=0.0,
                visible_seconds=1.0,
                max_frames=0,
            )


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
        self.assertTrue(all(marker["category"] == "beat" for marker in result.markers))
        self.assertTrue(all("meter" not in marker["metadata"] for marker in result.markers))

    def test_harmonic_change_markers_ignore_silent_chroma_frames(self):
        times = np.asarray([0.0, 1.0, 2.0, 3.0])
        chroma = np.zeros((12, 4))
        chroma[0, 0] = 1.0
        chroma[0, 2] = 1.0
        chroma[7, 3] = 1.0

        frames = _chroma_frames(times, chroma, max_frames=16)
        markers = _harmonic_change_markers(frames, max_markers=16)

        self.assertEqual(frames[1]["dominant_pitch_class"], -1)
        self.assertEqual([marker["timestamp"] for marker in markers], [3.0])

    def test_harmonic_change_markers_round_timestamps(self):
        markers = _harmonic_change_markers(
            [
                {"time": 0.0, "dominant_pitch_class": 0},
                {"time": 1.123456789, "dominant_pitch_class": 7},
            ],
            max_markers=16,
        )

        self.assertEqual(markers[0]["timestamp"], 1.123457)

    def test_invalid_numeric_settings_fail_before_audio_loading(self):
        missing_audio = Path("does-not-exist.wav")
        engine = MusicAnalysisEngine()

        with self.assertRaisesRegex(ValueError, "hop_length"):
            engine.analyze_rhythm(missing_audio, {"hop_length": 0})
        with self.assertRaisesRegex(ValueError, "max_frames"):
            engine.analyze_energy(missing_audio, {"max_frames": 0})
        with self.assertRaisesRegex(ValueError, "max_markers"):
            engine.analyze_harmony(missing_audio, {"max_markers": 0})

    def test_energy_profile_observes_cancellation_between_analysis_stages(self):
        cancelled = {"value": False}

        def fake_rms(*_args, **_kwargs):
            cancelled["value"] = True
            return np.asarray([[0.25, 0.75]])

        with (
            patch("autolight.analysis.music._load_audio", return_value=(np.ones(4096), 8000)),
            patch("autolight.analysis.music.librosa.feature.rms", side_effect=fake_rms),
            patch("autolight.analysis.music.librosa.onset.onset_strength") as onset_strength,
            self.assertRaises(MusicAnalysisCancelled),
        ):
            MusicAnalysisEngine.analyze_energy(
                Path("song.wav"),
                {"max_frames": 8},
                cancel_requested=lambda: cancelled["value"],
            )

        onset_strength.assert_not_called()

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
            self.assertIsInstance(result.payload, dict)
            json.dumps(result.payload, allow_nan=False)
            json.dumps(result.markers, allow_nan=False)
