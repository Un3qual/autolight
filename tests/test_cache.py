import tempfile
import unittest
from pathlib import Path

from autolight.cache.keys import canonical_hash, track_dependency_hash
from autolight.cache.store import CacheStore
from autolight.project.models import CacheEntry


class CacheTest(unittest.TestCase):
    def test_canonical_hash_is_order_stable(self):
        left = canonical_hash({"b": 2, "a": 1})
        right = canonical_hash({"a": 1, "b": 2})

        self.assertEqual(left, right)

    def test_canonical_hash_rejects_unsupported_values(self):
        with self.assertRaises(TypeError):
            canonical_hash({"value": object()})

    def test_canonical_hash_rejects_nan_values(self):
        with self.assertRaises(ValueError):
            canonical_hash({"value": float("nan")})

    def test_track_dependency_hash_includes_all_dependency_inputs(self):
        base = track_dependency_hash(
            input_cache_refs=["audio:abc"],
            transform_id="markers.beats",
            transform_version="1",
            params={"interval": 0.5},
        )
        different_input_refs = track_dependency_hash(
            input_cache_refs=["audio:def"],
            transform_id="markers.beats",
            transform_version="1",
            params={"interval": 0.5},
        )
        different_transform_id = track_dependency_hash(
            input_cache_refs=["audio:abc"],
            transform_id="markers.downbeats",
            transform_version="1",
            params={"interval": 0.5},
        )
        different_transform_version = track_dependency_hash(
            input_cache_refs=["audio:abc"],
            transform_id="markers.beats",
            transform_version="2",
            params={"interval": 0.5},
        )
        different_params = track_dependency_hash(
            input_cache_refs=["audio:abc"],
            transform_id="markers.beats",
            transform_version="1",
            params={"interval": 1.0},
        )

        self.assertNotEqual(base, different_input_refs)
        self.assertNotEqual(base, different_transform_id)
        self.assertNotEqual(base, different_transform_version)
        self.assertNotEqual(base, different_params)

    def test_cache_store_writes_artifact_and_reports_valid_entry(self):
        with tempfile.TemporaryDirectory() as tmp:
            store = CacheStore(Path(tmp))
            entry = store.write_bytes("markers", "dep_hash", b"[]", "1")

            self.assertTrue(store.artifact_path(entry).exists())
            self.assertTrue(store.is_entry_valid(entry))
            self.assertEqual(entry.artifact_kind, "markers")

    def test_cache_store_rejects_invalid_artifact_kind(self):
        with tempfile.TemporaryDirectory() as tmp:
            store = CacheStore(Path(tmp))

            for artifact_kind in ["", "../markers", "/markers", "markers/nested", "markers.beats"]:
                with self.subTest(artifact_kind=artifact_kind):
                    with self.assertRaises(ValueError):
                        store.write_bytes(artifact_kind, "dep_hash", b"[]", "1")

    def test_artifact_path_rejects_absolute_and_parent_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            store = CacheStore(Path(tmp))

            for path in ["/tmp/marker.bin", "../marker.bin", "markers/../marker.bin"]:
                with self.subTest(path=path):
                    with self.assertRaises(ValueError):
                        store.artifact_path(cache_entry(path))

    def test_artifact_path_rejects_symlink_escape(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / "cache"
            outside = Path(tmp) / "outside"
            root.mkdir()
            outside.mkdir()
            try:
                (root / "link").symlink_to(outside, target_is_directory=True)
            except OSError as error:
                self.skipTest(f"symlink unavailable: {error}")

            store = CacheStore(root)

            with self.assertRaises(ValueError):
                store.artifact_path(cache_entry("link/marker.bin"))

    def test_is_entry_valid_rejects_missing_or_wrong_size_artifacts(self):
        with tempfile.TemporaryDirectory() as tmp:
            store = CacheStore(Path(tmp))
            entry = store.write_bytes("markers", "dep_hash", b"[]", "1")
            path = store.artifact_path(entry)

            path.unlink()
            self.assertFalse(store.is_entry_valid(entry))

            path.write_bytes(b"[123]")
            self.assertFalse(store.is_entry_valid(entry))

    def test_is_entry_valid_rejects_directory_artifact_with_matching_size(self):
        with tempfile.TemporaryDirectory() as tmp:
            store = CacheStore(Path(tmp))
            artifact_dir = Path(tmp) / "markers"
            artifact_dir.mkdir()
            entry = cache_entry("markers")
            entry.size_bytes = artifact_dir.stat().st_size

            self.assertFalse(store.is_entry_valid(entry))

    def test_is_entry_valid_returns_false_for_invalid_persisted_path(self):
        with tempfile.TemporaryDirectory() as tmp:
            store = CacheStore(Path(tmp))

            self.assertFalse(store.is_entry_valid(cache_entry("../markers.bin")))


def cache_entry(path: str) -> CacheEntry:
    return CacheEntry(
        id="entry",
        dependency_hash="dep_hash",
        artifact_kind="markers",
        path=path,
        created_at="",
        transform_version="1",
        size_bytes=2,
    )


if __name__ == "__main__":
    unittest.main()
