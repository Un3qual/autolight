from __future__ import annotations

import hashlib
import json
import math
import os
import tempfile
from collections.abc import Callable
from dataclasses import asdict, is_dataclass
from enum import StrEnum
from pathlib import Path
from typing import Any
from uuid import uuid4

from autolight.cache.keys import canonical_hash
from autolight.project.audio_probe import probe_audio_file
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

MAX_RELINK_CANDIDATES = 2048
MAX_RELINK_DIRECTORIES = 512
MARKER_COLOR_PALETTE = {
    "cyan": "#67e8f9",
    "green": "#a7f3d0",
    "amber": "#fbbf24",
    "violet": "#c4b5fd",
    "rose": "#fda4af",
    "blue": "#93c5fd",
}
DEFAULT_MARKER_COLOR = "cyan"


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
    metadata = probe_audio_file(audio_path)
    asset = AudioAsset(
        id=new_id("asset"),
        path=str(audio_path),
        duration=metadata.duration,
        sample_rate=metadata.sample_rate,
        channels=metadata.channels,
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


def refresh_audio_asset_status(project: ProjectDocument, search_dirs: list[str | Path] | None = None) -> list[str]:
    changed_asset_ids: list[str] = []
    search_roots = [Path(root) for root in search_dirs or []]
    candidate_index: _RelinkCandidateIndex | None = None

    def find_replacement(fingerprint: str, hint: str) -> Path | None:
        nonlocal candidate_index
        if Path(hint).stem:
            if candidate_index is None:
                candidate_index = _RelinkCandidateIndex(search_roots)
            return candidate_index.find(fingerprint, hint)
        return None

    for asset in project.audio_assets:
        if _refresh_audio_asset(asset, find_replacement):
            changed_asset_ids.append(asset.id)

    return changed_asset_ids


def _refresh_audio_asset(asset: AudioAsset, find_replacement: Callable[[str, str], Path | None]) -> bool:
    asset_path = Path(asset.path)
    hint = asset_path.name

    if asset_path.is_file():
        if _fingerprint_matches(asset_path, asset.fingerprint):
            return _mark_audio_asset_online(asset)
        replacement = find_replacement(asset.fingerprint, hint)
        if replacement is not None:
            return _relink_audio_asset(asset, replacement)
        return _mark_audio_asset_modified(asset)

    replacement = find_replacement(asset.fingerprint, hint)
    if replacement is not None:
        return _relink_audio_asset(asset, replacement)

    return _mark_audio_asset_offline(asset, hint)


def _mark_audio_asset_online(asset: AudioAsset) -> bool:
    if asset.import_status == "online" and not asset.relink_hint:
        return False
    asset.import_status = "online"
    asset.relink_hint = ""
    return True


def _relink_audio_asset(asset: AudioAsset, replacement: Path) -> bool:
    if asset.path == str(replacement) and asset.import_status == "online" and not asset.relink_hint:
        return False
    asset.path = str(replacement)
    asset.import_status = "online"
    asset.relink_hint = ""
    return True


def _mark_audio_asset_offline(asset: AudioAsset, hint: str) -> bool:
    if asset.import_status == "offline" and asset.relink_hint == hint:
        return False
    asset.import_status = "offline"
    asset.relink_hint = hint
    return True


def _mark_audio_asset_modified(asset: AudioAsset) -> bool:
    if asset.import_status == "modified" and not asset.relink_hint:
        return False
    asset.import_status = "modified"
    asset.relink_hint = ""
    return True


def _fingerprint_matches(path: Path, fingerprint: str) -> bool:
    try:
        return fingerprint_file(path) == fingerprint
    except OSError:
        return False


class _RelinkCandidateIndex:
    def __init__(self, search_roots: list[Path]):
        self._candidates: list[tuple[str, Path]] = []
        self._fingerprints: dict[Path, str | None] = {}
        seen_roots: set[Path] = set()
        for root in search_roots:
            if root in seen_roots or not root.is_dir():
                continue
            seen_roots.add(root)
            for candidate in _iter_relink_candidates(root):
                self._candidates.append((candidate.stem.casefold(), candidate))

    def find(self, fingerprint: str, filename_hint: str) -> Path | None:
        hinted_stem = Path(filename_hint).stem.casefold()
        if not hinted_stem:
            return None
        for candidate_stem, candidate in self._candidates:
            if candidate_stem.startswith(hinted_stem) and self._fingerprint_matches(candidate, fingerprint):
                return candidate
        return None

    def _fingerprint_matches(self, path: Path, fingerprint: str) -> bool:
        if path not in self._fingerprints:
            try:
                self._fingerprints[path] = fingerprint_file(path)
            except OSError:
                self._fingerprints[path] = None
        return self._fingerprints[path] == fingerprint


def _iter_relink_candidates(root: Path):
    candidate_count = 0
    searched_directories = 0
    stack: list[Path] = [root]
    while stack and searched_directories < MAX_RELINK_DIRECTORIES:
        current = stack.pop()
        searched_directories += 1
        try:
            children = current.iterdir()
        except OSError:
            continue
        for child in children:
            if child.is_file():
                yield child
                candidate_count += 1
                if candidate_count >= MAX_RELINK_CANDIDATES:
                    return
            elif child.is_dir() and not child.is_symlink():
                stack.append(child)


def _find_relink_candidate(fingerprint: str, search_roots: list[Path], filename_hint: str) -> Path | None:
    return _RelinkCandidateIndex(search_roots).find(fingerprint, filename_hint)


def refresh_audio_track_status(project: ProjectDocument) -> list[str]:
    changed_track_ids: list[str] = []
    problem_assets = {asset.id: asset for asset in project.audio_assets if asset.import_status != "online"}
    previous_states = {track.id: track.result_state for track in project.tracks}
    problem_source_tracks: list[tuple[str, str]] = []
    restored_source_track_ids: list[str] = []

    for track in _source_tracks(project):
        track_changed, problem_error, restored = _refresh_source_audio_track(track, problem_assets)
        if track_changed:
            changed_track_ids.append(track.id)
        if problem_error:
            problem_source_tracks.append((track.id, problem_error))
        if restored:
            restored_source_track_ids.append(track.id)

    for track_id, problem_error in problem_source_tracks:
        mark_dependents_stale(project, track_id, error=_input_audio_problem_error(problem_error))

    for track_id in restored_source_track_ids:
        _restore_audio_problem_dependents(project, track_id)

    _append_changed_dependents(project, previous_states, changed_track_ids)

    return changed_track_ids


def _source_tracks(project: ProjectDocument) -> list[Track]:
    return [track for track in project.tracks if track.type == TrackType.SOURCE]


def _refresh_source_audio_track(
    track: Track,
    problem_assets: dict[str, AudioAsset],
) -> tuple[bool, str, bool]:
    asset = _problem_audio_asset_for_track(track, problem_assets)
    if asset is None:
        restored = _clear_audio_problem_error(track)
        return restored, "", restored
    error = _audio_problem_error(asset)
    return _mark_source_audio_problem(track, error), error, False


def _problem_audio_asset_for_track(
    track: Track,
    problem_assets: dict[str, AudioAsset],
) -> AudioAsset | None:
    asset_id = track.provenance.get("asset_id")
    if not isinstance(asset_id, str):
        return None
    return problem_assets.get(asset_id)


def _clear_audio_problem_error(track: Track) -> bool:
    if not _is_audio_problem_error(track.error):
        return False
    track.error = ""
    if track.result_state == ResultState.STALE:
        track.result_state = ResultState.COMPLETE
    return True


def _is_audio_problem_error(error: str) -> bool:
    return error.startswith("audio asset offline:") or error.startswith("audio asset modified:")


def _is_audio_dependency_error(error: str) -> bool:
    return error.startswith("input audio asset offline:") or error.startswith("input audio asset modified:")


def _mark_source_audio_problem(track: Track, error: str) -> bool:
    changed = False
    if track.result_state == ResultState.COMPLETE:
        track.result_state = ResultState.STALE
        changed = True
    if track.error != error:
        track.error = error
        changed = True
    return changed


def _input_audio_problem_error(source_error: str) -> str:
    return f"input {source_error}"


def _audio_problem_error(asset: AudioAsset) -> str:
    hint = asset.relink_hint or Path(asset.path).name or asset.id
    return f"audio asset {asset.import_status}: {hint}"


def _restore_audio_problem_dependents(project: ProjectDocument, restored_track_id: str) -> None:
    restored_ids = {restored_track_id}
    changed = True
    while changed:
        changed = False
        for track in project.tracks:
            if track.type == TrackType.SOURCE or track.id in restored_ids:
                continue
            if track.result_state != ResultState.STALE or not _is_audio_dependency_error(track.error):
                continue
            if not _all_inputs_complete(project, track):
                continue
            track.result_state = ResultState.COMPLETE
            track.error = ""
            restored_ids.add(track.id)
            changed = True


def _all_inputs_complete(project: ProjectDocument, track: Track) -> bool:
    return all(
        (input_track := find_track(project, input_id)) is not None
        and input_track.result_state == ResultState.COMPLETE
        for input_id in track.input_track_ids
    )


def _append_changed_dependents(
    project: ProjectDocument,
    previous_states: dict[str, ResultState],
    changed_track_ids: list[str],
) -> None:
    for track in project.tracks:
        if track.id not in changed_track_ids and track.result_state != previous_states[track.id]:
            changed_track_ids.append(track.id)


def track_dependency_inputs(project: ProjectDocument, track: Track) -> list[str]:
    if track.cache_refs:
        return list(track.cache_refs)
    return [f"track:{track.id}:{_track_content_hash(project, track)}"]


def _track_content_hash(project: ProjectDocument, track: Track) -> str:
    payload = {
        "track_id": track.id,
        "track_type": track.type.value,
        "input_track_ids": list(track.input_track_ids),
        "dependency_hash": track.dependency_hash,
        "provenance": track.provenance,
        "markers": [_to_json(marker) for marker in _markers_for_track(project, track.id)],
    }
    if track.type == TrackType.SOURCE:
        payload["audio_asset"] = _source_audio_asset_payload(project, track)
    return canonical_hash(payload)


def _markers_for_track(project: ProjectDocument, track_id: str) -> list[Marker]:
    return sorted(
        (marker for marker in project.markers if marker.track_id == track_id),
        key=lambda marker: (marker.timestamp, marker.id),
    )


def _source_audio_asset_payload(project: ProjectDocument, track: Track) -> dict[str, str]:
    asset_id = track.provenance.get("asset_id")
    asset = next(
        (candidate for candidate in project.audio_assets if isinstance(asset_id, str) and candidate.id == asset_id),
        None,
    )
    if asset is None:
        return {"asset_id": str(asset_id or ""), "status": "missing"}
    return {
        "asset_id": asset.id,
        "fingerprint": asset.fingerprint,
        "import_status": asset.import_status,
        "relink_hint": asset.relink_hint,
    }


def marker_display_color(marker: Marker) -> str:
    color = marker.metadata.get("color", "")
    if isinstance(color, str) and color in MARKER_COLOR_PALETTE:
        return MARKER_COLOR_PALETTE[color]
    return MARKER_COLOR_PALETTE[DEFAULT_MARKER_COLOR]


def _editable_track_or_raise(project: ProjectDocument, track_id: str) -> Track:
    track = find_track(project, track_id)
    if track is None:
        raise ValueError(f"track not found: {track_id}")
    if track.type != TrackType.EDITABLE:
        raise ValueError("markers can only be edited on an editable track")
    return track


def _editable_marker_or_raise(project: ProjectDocument, track_id: str, marker_id: str) -> Marker:
    for marker in project.markers:
        if marker.track_id == track_id and marker.id == marker_id:
            return marker
    raise ValueError(f"marker not found on track {track_id}: {marker_id}")


def _finite_marker_timestamp(timestamp: float) -> float:
    timestamp_value = float(timestamp)
    if not math.isfinite(timestamp_value):
        raise ValueError("marker timestamp must be finite")
    return timestamp_value


def _normalized_marker_color(color: str) -> str:
    value = str(color or DEFAULT_MARKER_COLOR).strip().lower()
    if value not in MARKER_COLOR_PALETTE:
        raise ValueError(f"marker color must be one of: {', '.join(MARKER_COLOR_PALETTE)}")
    return value


def _apply_marker_fields(
    marker: Marker,
    *,
    timestamp: float | None = None,
    label: str | None = None,
    category: str | None = None,
    color: str | None = None,
) -> bool:
    timestamp_value = _finite_marker_timestamp(timestamp) if timestamp is not None else None
    label_value = str(label) if label is not None else None
    category_value = str(category or "cue") if category is not None else None
    color_value = _normalized_marker_color(color) if color is not None else None

    changed = False
    if timestamp_value is not None:
        if marker.timestamp != timestamp_value:
            marker.timestamp = timestamp_value
            changed = True
    if label_value is not None:
        if marker.label != label_value:
            marker.label = label_value
            changed = True
    if category_value is not None:
        if marker.category != category_value:
            marker.category = category_value
            changed = True
    if color_value is not None:
        if marker.metadata.get("color") != color_value:
            marker.metadata["color"] = color_value
            changed = True
    return changed


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

    source_markers = {marker.id: marker for marker in project.markers if marker.track_id == source_track_id}
    selected_markers = []
    for marker_id in source_marker_ids:
        try:
            selected_markers.append(source_markers[marker_id])
        except KeyError as exc:
            raise ValueError(f"source marker not found on track {source_track_id}: {marker_id}") from exc

    track = Track(
        id=new_id("track"),
        type=TrackType.EDITABLE,
        name=name,
        input_track_ids=[source_track_id],
        result_state=ResultState.COMPLETE,
        provenance={"source_track_id": source_track_id, "source_marker_ids": list(source_marker_ids)},
    )
    project.tracks.append(track)
    for source_marker in selected_markers:
        project.markers.append(
            Marker(
                id=new_id("marker"),
                track_id=track.id,
                timestamp=source_marker.timestamp,
                duration=source_marker.duration,
                label=source_marker.label,
                category=source_marker.category,
                confidence=source_marker.confidence,
                tags=list(source_marker.tags),
                source_transform=source_marker.source_transform,
                source_marker_ids=[source_marker.id],
                metadata=dict(source_marker.metadata),
            )
        )
    return track


def update_editable_marker(
    project: ProjectDocument,
    track_id: str,
    marker_id: str,
    *,
    timestamp: float,
    label: str,
    category: str,
    color: str,
) -> Marker:
    _editable_track_or_raise(project, track_id)
    marker = _editable_marker_or_raise(project, track_id, marker_id)
    changed = _apply_marker_fields(
        marker,
        timestamp=timestamp,
        label=label,
        category=category,
        color=color,
    )
    if changed:
        mark_dependents_stale(project, track_id)
    return marker


def bulk_update_editable_markers(
    project: ProjectDocument,
    track_id: str,
    marker_ids: list[str],
    *,
    label: str,
    category: str,
    color: str,
) -> int:
    _editable_track_or_raise(project, track_id)
    selected_ids = set(marker_ids)
    changed_count = 0
    for marker in project.markers:
        if marker.track_id != track_id:
            continue
        if selected_ids and marker.id not in selected_ids:
            continue
        if _apply_marker_fields(marker, label=label, category=category, color=color):
            changed_count += 1
    if changed_count:
        mark_dependents_stale(project, track_id)
    return changed_count


def add_editable_marker(project: ProjectDocument, track_id: str, timestamp: float, label: str) -> Marker:
    track = find_track(project, track_id)
    if track is None:
        raise ValueError(f"track not found: {track_id}")
    if track.type != TrackType.EDITABLE:
        raise ValueError("markers can only be added to an editable track")
    timestamp_value = _finite_marker_timestamp(timestamp)
    marker = Marker(
        id=new_id("marker"),
        track_id=track_id,
        timestamp=timestamp_value,
        label=str(label),
        category="cue",
        metadata={"created_by": "user", "color": DEFAULT_MARKER_COLOR},
    )
    project.markers.append(marker)
    mark_dependents_stale(project, track_id)
    return marker


def delete_editable_marker(project: ProjectDocument, track_id: str, marker_id: str) -> bool:
    track = find_track(project, track_id)
    if track is None:
        raise ValueError(f"track not found: {track_id}")
    if track.type != TrackType.EDITABLE:
        raise ValueError("markers can only be deleted from an editable track")
    before = len(project.markers)
    project.markers[:] = [
        marker for marker in project.markers if not (marker.track_id == track_id and marker.id == marker_id)
    ]
    deleted = len(project.markers) != before
    if deleted:
        mark_dependents_stale(project, track_id)
    return deleted


def find_track(project: ProjectDocument, track_id: str) -> Track | None:
    return next((track for track in project.tracks if track.id == track_id), None)


def validate_graph(project: ProjectDocument) -> None:
    audio_asset_ids = {asset.id for asset in project.audio_assets}
    if len(audio_asset_ids) != len(project.audio_assets):
        raise ValueError("duplicate audio asset id")
    track_ids = {track.id for track in project.tracks}
    if len(track_ids) != len(project.tracks):
        raise ValueError("duplicate track id")
    cache_entry_ids = {entry.id for entry in project.cache_entries}
    if len(cache_entry_ids) != len(project.cache_entries):
        raise ValueError("duplicate cache entry id")
    if len({marker.id for marker in project.markers}) != len(project.markers):
        raise ValueError("duplicate marker id")
    if len({run.id for run in project.job_runs}) != len(project.job_runs):
        raise ValueError("duplicate job run id")

    for track in project.tracks:
        if track.type == TrackType.SOURCE and track.input_track_ids:
            raise ValueError("source tracks cannot have inputs")
        if track.type == TrackType.SOURCE:
            if not isinstance(track.provenance, dict):
                raise ValueError(f"source track provenance must be a mapping: {track.id}")
            asset_id = track.provenance.get("asset_id")
            if not isinstance(asset_id, str) or asset_id not in audio_asset_ids:
                raise ValueError(f"source track references missing audio asset: {track.id}")
        if track.type == TrackType.GENERATED and len(track.input_track_ids) != 1:
            raise ValueError("generated tracks must have exactly one input")
        for input_id in track.input_track_ids:
            if input_id not in track_ids:
                raise ValueError(f"missing input track: {input_id}")
        for cache_ref in track.cache_refs:
            if cache_ref not in cache_entry_ids:
                raise ValueError(f"track cache ref not found: {cache_ref}")

    for marker in project.markers:
        if marker.track_id not in track_ids:
            raise ValueError(f"marker references missing track: {marker.track_id}")

    for run in project.job_runs:
        if run.track_id not in track_ids:
            raise ValueError(f"job run references missing track: {run.track_id}")
        for cache_ref in run.produced_cache_refs:
            if cache_ref not in cache_entry_ids:
                raise ValueError(f"job run cache ref not found: {cache_ref}")

    _validate_acyclic(project)


def mark_dependents_stale(project: ProjectDocument, changed_track_id: str, error: str = "") -> None:
    changed = True
    stale_ids = {changed_track_id}
    while changed:
        changed = False
        for track in project.tracks:
            if track.type == TrackType.SOURCE:
                continue
            if track.id in stale_ids:
                continue
            if any(input_id in stale_ids for input_id in track.input_track_ids):
                was_complete = track.result_state == ResultState.COMPLETE
                track.result_state = ResultState.STALE
                if error and (was_complete or _is_audio_dependency_error(track.error)):
                    track.error = error
                stale_ids.add(track.id)
                changed = True


class ProjectStore:
    @staticmethod
    def save(project: ProjectDocument, path: str | Path) -> None:
        validate_graph(project)
        target = Path(path)
        target.parent.mkdir(parents=True, exist_ok=True)
        payload = json.dumps(_to_json(project), indent=2, sort_keys=True)
        descriptor, temp_name = tempfile.mkstemp(
            prefix=f".{target.name}.",
            suffix=".tmp",
            dir=target.parent,
        )
        temp_path = Path(temp_name)
        try:
            with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
                handle.write(payload)
            os.replace(temp_path, target)
            temp_path = None
        finally:
            if temp_path is not None:
                temp_path.unlink(missing_ok=True)

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
