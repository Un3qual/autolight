from __future__ import annotations

import hashlib
import json
from dataclasses import asdict, is_dataclass
from enum import StrEnum
from pathlib import Path
from typing import Any
from uuid import uuid4

from autolight.project.models import (
    AudioAsset,
    CacheEntry,
    JobRun,
    Marker,
    ProjectDocument,
    ResultState,
    SCHEMA_VERSION,
    Track,
    TrackType,
)


def new_id(prefix: str) -> str:
    return f"{prefix}_{uuid4().hex[:12]}"


def new_project(name: str) -> ProjectDocument:
    return ProjectDocument(id=new_id("project"), name=name)


def fingerprint_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def import_audio_asset(project: ProjectDocument, path: str | Path) -> Track:
    audio_path = Path(path)
    if audio_path.exists() and not audio_path.is_file():
        raise IsADirectoryError(f"audio asset path is not a file: {audio_path}")
    if not audio_path.is_file():
        raise FileNotFoundError(str(audio_path))

    asset = AudioAsset(
        id=new_id("asset"),
        path=str(audio_path),
        duration=0.0,
        sample_rate=0,
        channels=0,
        fingerprint=fingerprint_file(audio_path),
    )
    track = Track(
        id=new_id("track"),
        type=TrackType.SOURCE,
        name=audio_path.stem,
        result_state=ResultState.COMPLETE,
        provenance={"asset_id": asset.id},
    )
    project.audio_assets.append(asset)
    project.tracks.append(track)
    return track


def add_generated_track(
    project: ProjectDocument,
    parent_track_id: str,
    name: str,
    transform_id: str,
    transform_params: dict[str, Any],
    transform_version: str,
    output_schema: str,
    dependency_hash: str,
) -> Track:
    if find_track(project, parent_track_id) is None:
        raise ValueError(f"parent track not found: {parent_track_id}")

    track = Track(
        id=new_id("track"),
        type=TrackType.GENERATED,
        name=name,
        input_track_ids=[parent_track_id],
        transform_id=transform_id,
        transform_params=dict(transform_params),
        transform_version=transform_version,
        output_schema=output_schema,
        dependency_hash=dependency_hash,
    )
    project.tracks.append(track)
    return track


def create_editable_track_from_markers(
    project: ProjectDocument,
    source_track_id: str,
    name: str,
    source_marker_ids: list[str],
) -> Track:
    if find_track(project, source_track_id) is None:
        raise ValueError(f"source track not found: {source_track_id}")

    track = Track(
        id=new_id("track"),
        type=TrackType.EDITABLE,
        name=name,
        input_track_ids=[source_track_id],
        result_state=ResultState.COMPLETE,
        provenance={"source_track_id": source_track_id, "source_marker_ids": list(source_marker_ids)},
    )
    project.tracks.append(track)
    return track


def find_track(project: ProjectDocument, track_id: str) -> Track | None:
    return next((track for track in project.tracks if track.id == track_id), None)


def validate_graph(project: ProjectDocument) -> None:
    track_ids = {track.id for track in project.tracks}
    if len(track_ids) != len(project.tracks):
        raise ValueError("duplicate track id")

    for track in project.tracks:
        if track.type == TrackType.SOURCE and track.input_track_ids:
            raise ValueError("source tracks cannot have inputs")
        if track.type == TrackType.GENERATED and len(track.input_track_ids) != 1:
            raise ValueError("generated tracks must have exactly one input")
        for input_id in track.input_track_ids:
            if input_id not in track_ids:
                raise ValueError(f"missing input track: {input_id}")

    _validate_acyclic(project)


def mark_dependents_stale(project: ProjectDocument, changed_track_id: str) -> None:
    changed = True
    stale_ids = {changed_track_id}
    while changed:
        changed = False
        for track in project.tracks:
            if track.type != TrackType.GENERATED:
                continue
            if track.id in stale_ids:
                continue
            if any(input_id in stale_ids for input_id in track.input_track_ids):
                track.result_state = ResultState.STALE
                stale_ids.add(track.id)
                changed = True


class ProjectStore:
    @staticmethod
    def save(project: ProjectDocument, path: str | Path) -> None:
        validate_graph(project)
        target = Path(path)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(_to_json(project), indent=2, sort_keys=True), encoding="utf-8")

    @staticmethod
    def load(path: str | Path) -> ProjectDocument:
        raw = json.loads(Path(path).read_text(encoding="utf-8"))
        if raw.get("schema_version") != SCHEMA_VERSION:
            raise ValueError(f"unsupported schema version: {raw.get('schema_version')}")

        project = _project_from_json(raw)
        validate_graph(project)
        return project


def _to_json(value: Any) -> Any:
    if isinstance(value, StrEnum):
        return value.value
    if is_dataclass(value):
        return {key: _to_json(item) for key, item in asdict(value).items()}
    if isinstance(value, list):
        return [_to_json(item) for item in value]
    if isinstance(value, dict):
        return {key: _to_json(item) for key, item in value.items()}
    return value


def _validate_acyclic(project: ProjectDocument) -> None:
    track_by_id = {track.id: track for track in project.tracks}
    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(track_id: str) -> None:
        if track_id in visiting:
            raise ValueError(f"cycle detected in track graph: {track_id}")
        if track_id in visited:
            return

        visiting.add(track_id)
        for input_id in track_by_id[track_id].input_track_ids:
            visit(input_id)
        visiting.remove(track_id)
        visited.add(track_id)

    for track in project.tracks:
        visit(track.id)


def _project_from_json(raw: dict[str, Any]) -> ProjectDocument:
    return ProjectDocument(
        id=raw["id"],
        name=raw["name"],
        schema_version=raw["schema_version"],
        audio_assets=[AudioAsset(**item) for item in raw["audio_assets"]],
        tracks=[
            Track(
                **{
                    **item,
                    "type": TrackType(item["type"]),
                    "result_state": ResultState(item["result_state"]),
                }
            )
            for item in raw["tracks"]
        ],
        markers=[Marker(**item) for item in raw["markers"]],
        job_runs=[
            JobRun(
                **{
                    **item,
                    "state": ResultState(item["state"]),
                }
            )
            for item in raw["job_runs"]
        ],
        cache_entries=[CacheEntry(**item) for item in raw["cache_entries"]],
        ui_state=raw["ui_state"],
    )
