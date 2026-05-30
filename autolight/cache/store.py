from __future__ import annotations

from datetime import datetime, timezone
from pathlib import Path

from autolight.cache.keys import canonical_hash
from autolight.project.models import CacheEntry


class CacheStore:
    def __init__(self, root: Path):
        self.root = root
        self.root.mkdir(parents=True, exist_ok=True)

    def write_bytes(
        self,
        artifact_kind: str,
        dependency_hash: str,
        payload: bytes,
        transform_version: str,
    ) -> CacheEntry:
        entry_id = canonical_hash({"kind": artifact_kind, "dependency": dependency_hash, "payload": payload.hex()})[:16]
        relative_path = Path(artifact_kind) / f"{entry_id}.bin"
        target = self.root / relative_path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(payload)
        return CacheEntry(
            id=entry_id,
            dependency_hash=dependency_hash,
            artifact_kind=artifact_kind,
            path=str(relative_path),
            created_at=datetime.now(timezone.utc).isoformat(),
            transform_version=transform_version,
            size_bytes=len(payload),
        )

    def artifact_path(self, entry: CacheEntry) -> Path:
        return self.root / entry.path

    def is_entry_valid(self, entry: CacheEntry) -> bool:
        path = self.artifact_path(entry)
        return path.exists() and path.stat().st_size == entry.size_bytes
