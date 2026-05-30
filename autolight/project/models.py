from __future__ import annotations

from dataclasses import dataclass, field
from enum import StrEnum
from typing import Any


SCHEMA_VERSION = 1


class TrackType(StrEnum):
    SOURCE = "source"
    GENERATED = "generated"
    EDITABLE = "editable"


class ResultState(StrEnum):
    PENDING = "pending"
    RUNNING = "running"
    COMPLETE = "complete"
    STALE = "stale"
    FAILED = "failed"
    CANCELLED = "cancelled"
    BLOCKED = "blocked"


@dataclass(slots=True)
class AudioAsset:
    id: str
    path: str
    duration: float
    sample_rate: int
    channels: int
    fingerprint: str
    import_status: str = "online"
    relink_hint: str = ""


@dataclass(slots=True)
class Track:
    id: str
    type: TrackType
    name: str
    input_track_ids: list[str] = field(default_factory=list)
    transform_id: str = ""
    transform_params: dict[str, Any] = field(default_factory=dict)
    transform_version: str = ""
    output_schema: str = ""
    dependency_hash: str = ""
    result_state: ResultState = ResultState.PENDING
    cache_refs: list[str] = field(default_factory=list)
    provenance: dict[str, Any] = field(default_factory=dict)
    error: str = ""


@dataclass(slots=True)
class Marker:
    id: str
    track_id: str
    timestamp: float
    duration: float | None = None
    label: str = ""
    category: str = ""
    confidence: float | None = None
    tags: list[str] = field(default_factory=list)
    source_transform: str = ""
    source_marker_ids: list[str] = field(default_factory=list)
    metadata: dict[str, Any] = field(default_factory=dict)


@dataclass(slots=True)
class JobRun:
    id: str
    track_id: str
    transform_id: str
    parameters_hash: str
    state: ResultState = ResultState.PENDING
    progress: float = 0.0
    started_at: str = ""
    completed_at: str = ""
    error: str = ""
    produced_cache_refs: list[str] = field(default_factory=list)


@dataclass(slots=True)
class CacheEntry:
    id: str
    dependency_hash: str
    artifact_kind: str
    path: str
    created_at: str
    transform_version: str
    size_bytes: int = 0
    validation_status: str = "valid"


@dataclass(slots=True)
class ProjectDocument:
    id: str
    name: str
    schema_version: int = SCHEMA_VERSION
    audio_assets: list[AudioAsset] = field(default_factory=list)
    tracks: list[Track] = field(default_factory=list)
    markers: list[Marker] = field(default_factory=list)
    job_runs: list[JobRun] = field(default_factory=list)
    cache_entries: list[CacheEntry] = field(default_factory=list)
    ui_state: dict[str, Any] = field(default_factory=dict)
