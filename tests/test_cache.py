import tempfile
import unittest
from pathlib import Path

from autolight.cache.keys import canonical_hash, track_dependency_hash
from autolight.cache.store import CacheStore


class CacheTest(unittest.TestCase):
    def test_canonical_hash_is_order_stable(self):
        left = canonical_hash({"b": 2, "a": 1})
        right = canonical_hash({"a": 1, "b": 2})

        self.assertEqual(left, right)

    def test_track_dependency_hash_includes_parent_transform_and_params(self):
        first = track_dependency_hash(
            input_cache_refs=["audio:abc"],
            transform_id="markers.beats",
            transform_version="1",
            params={"interval": 0.5},
        )
        second = track_dependency_hash(
            input_cache_refs=["audio:abc"],
            transform_id="markers.beats",
            transform_version="2",
            params={"interval": 0.5},
        )

        self.assertNotEqual(first, second)

    def test_cache_store_writes_artifact_and_reports_valid_entry(self):
        with tempfile.TemporaryDirectory() as tmp:
            store = CacheStore(Path(tmp))
            entry = store.write_bytes("markers", "dep_hash", b"[]", "1")

            self.assertTrue(store.artifact_path(entry).exists())
            self.assertTrue(store.is_entry_valid(entry))
            self.assertEqual(entry.artifact_kind, "markers")


if __name__ == "__main__":
    unittest.main()
