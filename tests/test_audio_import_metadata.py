import math
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import soundfile

from autolight.project.audio_probe import probe_audio_file
from autolight.project.models import AudioAsset
from autolight.project.store import import_audio_asset, new_project
from tests.helpers import write_wav


class FakeAudioRead:
    channels = 2
    samplerate = 44100
    duration = 0.1

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, traceback):
        return False


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
        with tempfile.TemporaryDirectory() as tmp, self.assertRaisesRegex(IsADirectoryError, "not a file"):
            probe_audio_file(Path(tmp))

    def test_probe_audio_file_falls_back_when_soundfile_rejects_container(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.m4a"
            audio_path.write_bytes(b"decoder is mocked")

            with (
                patch("autolight.project.audio_probe.soundfile.info", side_effect=soundfile.SoundFileError("unsupported")),
                patch("audioread.audio_open", return_value=FakeAudioRead()) as open_audio,
                patch("librosa.load", side_effect=AssertionError("metadata probe must not decode full audio")),
            ):
                metadata = probe_audio_file(audio_path)

        open_audio.assert_called_once_with(str(audio_path))
        self.assertTrue(math.isclose(metadata.duration, 0.1, rel_tol=0.01))
        self.assertEqual(metadata.sample_rate, 44100)
        self.assertEqual(metadata.channels, 2)

    def test_probe_audio_file_preserves_non_format_errors(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path)

            with (
                patch("autolight.project.audio_probe.soundfile.info", side_effect=PermissionError("denied")),
                patch("audioread.audio_open") as open_audio,
                self.assertRaises(PermissionError),
            ):
                probe_audio_file(audio_path)

        open_audio.assert_not_called()

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

    def test_refresh_audio_asset_status_marks_existing_fingerprint_mismatch_offline(self):
        with tempfile.TemporaryDirectory() as tmp:
            audio_path = Path(tmp) / "song.wav"
            write_wav(audio_path, frames=8000)
            project = new_project("Demo")
            import_audio_asset(project, audio_path)
            write_wav(audio_path, frames=4000)

            from autolight.project.store import refresh_audio_asset_status

            changed_ids = refresh_audio_asset_status(project)

        self.assertEqual(changed_ids, [project.audio_assets[0].id])
        self.assertEqual(project.audio_assets[0].path, str(audio_path))
        self.assertEqual(project.audio_assets[0].import_status, "offline")
        self.assertEqual(project.audio_assets[0].relink_hint, "song.wav")

    def test_refresh_audio_asset_status_relinks_when_original_file_is_unreadable(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            old_path = root / "song.wav"
            replacement_path = root / "song-copy.wav"
            write_wav(old_path)
            payload = old_path.read_bytes()
            project = new_project("Demo")
            import_audio_asset(project, old_path)
            replacement_path.write_bytes(payload)

            from autolight.project import store as project_store

            original_fingerprint_file = project_store.fingerprint_file

            def maybe_unreadable(path):
                if Path(path) == old_path:
                    raise PermissionError("denied")
                return original_fingerprint_file(path)

            with patch.object(project_store, "fingerprint_file", side_effect=maybe_unreadable):
                changed_ids = project_store.refresh_audio_asset_status(project, search_dirs=[root])

        self.assertEqual(changed_ids, [project.audio_assets[0].id])
        self.assertEqual(project.audio_assets[0].path, str(replacement_path))
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

    def test_refresh_audio_asset_status_skips_relink_search_without_filename_hint(self):
        from autolight.project.store import refresh_audio_asset_status

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_wav(root / "candidate.wav")
            project = new_project("Demo")
            project.audio_assets.append(
                AudioAsset(
                    id="asset_missing_hint",
                    path="",
                    duration=0.0,
                    sample_rate=0,
                    channels=0,
                    fingerprint="missing",
                    import_status="online",
                )
            )

            with patch("autolight.project.store.fingerprint_file", return_value="other") as fingerprint_file:
                changed_ids = refresh_audio_asset_status(project, search_dirs=[root])

        self.assertEqual(changed_ids, ["asset_missing_hint"])
        self.assertEqual(project.audio_assets[0].import_status, "offline")
        self.assertEqual(project.audio_assets[0].relink_hint, "")
        fingerprint_file.assert_not_called()


if __name__ == "__main__":
    unittest.main()
