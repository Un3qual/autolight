from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable


ProgressCallback = Callable[[float], None]
CancelPredicate = Callable[[], bool]
TransformRunner = Callable[["TransformContext", dict[str, Any]], "TransformResult"]


class TransformCancelled(Exception):
    """Raised when a transform stops because cancellation was requested."""


@dataclass(slots=True)
class TransformContext:
    artifact_dir: Path
    cancel_requested: CancelPredicate
    progress: ProgressCallback


@dataclass(slots=True)
class TransformResult:
    markers: list[dict[str, Any]] = field(default_factory=list)
    artifacts: dict[str, str] = field(default_factory=dict)
    metadata: dict[str, Any] = field(default_factory=dict)


@dataclass(slots=True)
class TransformSpec:
    id: str
    version: str
    name: str
    input_schema: str
    output_schema: str
    estimated_cost: str
    run: TransformRunner


class TransformRegistry:
    def __init__(self):
        self._transforms: dict[str, TransformSpec] = {}

    def register(self, spec: TransformSpec) -> None:
        if spec.id in self._transforms:
            raise ValueError(f"duplicate transform id: {spec.id}")
        self._transforms[spec.id] = spec

    def get(self, transform_id: str, version: str | None = None) -> TransformSpec:
        try:
            spec = self._transforms[transform_id]
        except KeyError as exc:
            raise KeyError(f"unknown transform id: {transform_id}") from exc
        if version is not None and spec.version != version:
            raise ValueError(
                f"transform {transform_id} version mismatch: requested {version}, available {spec.version}"
            )
        return spec

    def ids(self) -> list[str]:
        return sorted(self._transforms)
