from __future__ import annotations

import hashlib
import re
import shutil
from datetime import datetime, timezone
from pathlib import Path

from autolight.cache.keys import canonical_hash
from autolight.project.models import CacheEntry


ARTIFACT_KIND_PATTERN = re.compile(r"^[A-Za-z0-9_-]+$")


class CacheStore:
    def __init__(self, root: Path):
        self.root = root
        self.root.mkdir(parents=True, exist_ok=True)
        self._resolved_root = self.root.resolve()

    def write_bytes(
        self,
        artifact_kind: str,
        dependency_hash: str,
        payload: bytes,
        transform_version: str,
    ) -> CacheEntry:
        self._validate_artifact_kind(artifact_kind)
        payload_digest = hashlib.sha256(payload).hexdigest()
        entry, target = self._entry_and_target(
            artifact_kind,
            dependency_hash,
            payload_digest,
            transform_version,
            len(payload),
        )
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(payload)
        return entry

    def write_file(
        self,
        artifact_kind: str,
        dependency_hash: str,
        source_path: str | Path,
        transform_version: str,
    ) -> CacheEntry:
        self._validate_artifact_kind(artifact_kind)
        source = Path(source_path)
        digest = hashlib.sha256()
        size_bytes = 0
        with source.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size_bytes += len(chunk)
                digest.update(chunk)

        entry, target = self._entry_and_target(
            artifact_kind,
            dependency_hash,
            digest.hexdigest(),
            transform_version,
            size_bytes,
        )
        target.parent.mkdir(parents=True, exist_ok=True)
        with source.open("rb") as input_file, target.open("wb") as output_file:
            shutil.copyfileobj(input_file, output_file, length=1024 * 1024)
        return entry

    def _entry_and_target(
        self,
        artifact_kind: str,
        dependency_hash: str,
        payload_digest: str,
        transform_version: str,
        size_bytes: int,
    ) -> tuple[CacheEntry, Path]:
        entry_id = canonical_hash(
            {"kind": artifact_kind, "dependency": dependency_hash, "payload_digest": payload_digest}
        )[:16]
        relative_path = Path(artifact_kind) / f"{entry_id}.bin"
        target = self._path_under_root(relative_path)
        return CacheEntry(
            id=entry_id,
            dependency_hash=dependency_hash,
            artifact_kind=artifact_kind,
            path=str(relative_path),
            created_at=datetime.now(timezone.utc).isoformat(),
            transform_version=transform_version,
            size_bytes=size_bytes,
        ), target

    def artifact_path(self, entry: CacheEntry) -> Path:
        return self._path_under_root(Path(entry.path))

    def is_entry_valid(self, entry: CacheEntry) -> bool:
        try:
            path = self.artifact_path(entry)
            return path.is_file() and path.stat().st_size == entry.size_bytes
        except (OSError, ValueError):
            return False

    def _validate_artifact_kind(self, artifact_kind: str) -> None:
        if not ARTIFACT_KIND_PATTERN.fullmatch(artifact_kind):
            raise ValueError(f"invalid artifact kind: {artifact_kind!r}")

    def _path_under_root(self, relative_path: Path) -> Path:
        if relative_path.is_absolute():
            raise ValueError(f"cache artifact path must be relative: {relative_path}")
        if not relative_path.parts or any(part in {"", ".", ".."} for part in relative_path.parts):
            raise ValueError(f"cache artifact path contains invalid components: {relative_path}")

        resolved_path = (self.root / relative_path).resolve()
        try:
            resolved_path.relative_to(self._resolved_root)
        except ValueError as error:
            raise ValueError(f"cache artifact path escapes cache root: {relative_path}") from error
        return resolved_path
