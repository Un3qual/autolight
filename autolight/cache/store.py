from __future__ import annotations

import hashlib
import os
import re
import tempfile
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
        self._atomic_write_bytes(target, payload)
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
        payload_digest, size_bytes, temp_path = self._copy_source_to_temp(source)

        try:
            entry, target = self._entry_and_target(
                artifact_kind,
                dependency_hash,
                payload_digest,
                transform_version,
                size_bytes,
            )
            target.parent.mkdir(parents=True, exist_ok=True)
            os.replace(temp_path, target)
            temp_path = None
            return entry
        finally:
            if temp_path is not None:
                temp_path.unlink(missing_ok=True)

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
            payload_digest=payload_digest,
        ), target

    def artifact_path(self, entry: CacheEntry) -> Path:
        return self._path_under_root(Path(entry.path))

    def is_entry_valid(self, entry: CacheEntry) -> bool:
        try:
            path = self.artifact_path(entry)
            if not entry.payload_digest or not path.is_file():
                return False
            if path.stat().st_size != entry.size_bytes:
                return False
            payload_digest = self._hash_file(path)
            expected_id = canonical_hash(
                {
                    "kind": entry.artifact_kind,
                    "dependency": entry.dependency_hash,
                    "payload_digest": payload_digest,
                }
            )[:16]
            return payload_digest == entry.payload_digest and entry.id == expected_id
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

    def _atomic_write_bytes(self, target: Path, payload: bytes) -> None:
        descriptor, temp_name = tempfile.mkstemp(
            prefix=f".{target.name}.",
            suffix=".tmp",
            dir=target.parent,
        )
        temp_path = Path(temp_name)
        try:
            with os.fdopen(descriptor, "wb") as handle:
                handle.write(payload)
            os.replace(temp_path, target)
            temp_path = None
        finally:
            if temp_path is not None:
                temp_path.unlink(missing_ok=True)

    def _copy_source_to_temp(self, source: Path) -> tuple[str, int, Path]:
        temp_dir = self.root / ".tmp"
        temp_dir.mkdir(parents=True, exist_ok=True)
        descriptor, temp_name = tempfile.mkstemp(prefix="artifact-", suffix=".tmp", dir=temp_dir)
        temp_path = Path(temp_name)
        digest = hashlib.sha256()
        size_bytes = 0
        try:
            with os.fdopen(descriptor, "wb") as output_file, source.open("rb") as input_file:
                for chunk in iter(lambda: input_file.read(1024 * 1024), b""):
                    size_bytes += len(chunk)
                    digest.update(chunk)
                    output_file.write(chunk)
            return digest.hexdigest(), size_bytes, temp_path
        except Exception:
            temp_path.unlink(missing_ok=True)
            raise

    def _hash_file(self, path: Path) -> str:
        digest = hashlib.sha256()
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
        return digest.hexdigest()
