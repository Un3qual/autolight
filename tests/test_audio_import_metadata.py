import math
import tempfile
import unittest
import wave
from pathlib import Path
from unittest.mock import patch

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

    def test_refresh_audio_asset_status_marks_missing_files_offline(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)
            project = new_project("Demo")
            import_audio_asset(project, audio_path)
            audio_path.unlink()

            from autolight.project.store import refresh_audio_asset_status

            changed_ids = refresh_audio_asset_status(project)

        self.assertEqual(changed_ids, [project.audio_assets[0].id])
        self.assertEqual(project.audio_assets[0].import_status, "offline")
        self.assertEqual(project.audio_assets[0].relink_hint, "song.wav")

    def test_refresh_audio_asset_status_relinks_matching_fingerprint(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            old_path = root / "old.wav"
            new_path = root / "old-copy.wav"
            write_wav(old_path)
            payload = old_path.read_bytes()
            project = new_project("Demo")
            import_audio_asset(project, old_path)
            old_path.unlink()
            new_path.write_bytes(payload)

            from autolight.project.store import refresh_audio_asset_status

            changed_ids = refresh_audio_asset_status(project, search_dirs=[root])

        self.assertEqual(changed_ids, [project.audio_assets[0].id])
        self.assertEqual(project.audio_assets[0].path, str(new_path))
        self.assertEqual(project.audio_assets[0].import_status, "online")
        self.assertEqual(project.audio_assets[0].relink_hint, "")

    def test_relink_candidate_hashes_only_name_prefix_matches(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            old_path = root / "song.wav"
            unrelated_path = root / "unrelated.wav"
            replacement_path = root / "song-remastered.wav"
            write_wav(old_path)
            payload = old_path.read_bytes()
            write_wav(unrelated_path)
            project = new_project("Demo")
            import_audio_asset(project, old_path)
            old_path.unlink()
            unrelated_path.write_bytes(payload)
            replacement_path.write_bytes(payload)

            from autolight.project import store as project_store

            checked_paths = []
            original_fingerprint_file = project_store.fingerprint_file

            def record_fingerprint(path):
                checked_paths.append(Path(path))
                return original_fingerprint_file(path)

            with patch.object(project_store, "fingerprint_file", side_effect=record_fingerprint):
                from autolight.project.store import refresh_audio_asset_status

                refresh_audio_asset_status(project, search_dirs=[root])

        self.assertIn(replacement_path, checked_paths)
        self.assertNotIn(unrelated_path, checked_paths)


if __name__ == "__main__":
    unittest.main()
