from __future__ import annotations

import hashlib
import json
from typing import Any


def canonical_hash(payload: Any) -> str:
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":"), default=str).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def track_dependency_hash(
    input_cache_refs: list[str],
    transform_id: str,
    transform_version: str,
    params: dict[str, Any],
) -> str:
    return canonical_hash(
        {
            "input_cache_refs": input_cache_refs,
            "transform_id": transform_id,
            "transform_version": transform_version,
            "params": params,
        }
    )
