from __future__ import annotations

from collections import deque
from dataclasses import dataclass
from pathlib import Path

from autolight.cache.store import CacheStore
from autolight.project.models import ProjectDocument, ResultState, Track, TrackType
from autolight.project.store import find_track


GENERATED_AUDIO_OUTPUT_SCHEMAS = {"artifact.audio.v1", "artifact.stem.v1"}


@dataclass(slots=True)
class TransformInputResolver:
    project: ProjectDocument
    cache_store: CacheStore

    def audio_path_for_track(self, track: Track) -> str:
        if track.type == TrackType.SOURCE:
            return self._source_audio_path(track)
        if track.type == TrackType.EDITABLE:
            return self._editable_source_audio_path(track)
        if track.type == TrackType.GENERATED:
            if track.result_state != ResultState.COMPLETE:
                raise ValueError(f"parent track is not complete: {track.name}")
            return str(self._valid_audio_artifact_path(track))
        raise ValueError(f"unsupported audio input track: {track.name}")

    def _source_audio_path(self, track: Track) -> str:
        asset_id = track.provenance.get("asset_id")
        for asset in self.project.audio_assets:
            if asset.id == asset_id:
                if asset.import_status != "online":
                    raise ValueError(f"source audio is not online: {track.name}")
                if not Path(asset.path).is_file():
                    raise ValueError(f"source audio path is missing: {asset.path}")
                return asset.path
        raise ValueError(f"source audio path not found for track: {track.name}")

    def _editable_source_audio_path(self, track: Track) -> str:
        if track.result_state != ResultState.COMPLETE:
            raise ValueError(f"parent track is not complete: {track.name}")

        first_source_error: ValueError | None = None
        for candidate in self._source_lineage_tracks(track):
            try:
                if candidate.type == TrackType.GENERATED:
                    if (
                        candidate.output_schema in GENERATED_AUDIO_OUTPUT_SCHEMAS
                        and candidate.result_state != ResultState.COMPLETE
                    ):
                        continue
                    return str(self._valid_audio_artifact_path(candidate))
                return self._source_audio_path(candidate)
            except ValueError as error:
                if (
                    candidate.type == TrackType.GENERATED
                    and candidate.output_schema in GENERATED_AUDIO_OUTPUT_SCHEMAS
                    and candidate.result_state == ResultState.COMPLETE
                ):
                    raise
                if first_source_error is None:
                    first_source_error = error

        if first_source_error is not None:
            raise first_source_error
        raise ValueError(f"editable track has no source audio context: {track.name}")

    def _source_lineage_tracks(self, track: Track):
        visited: set[str] = set()
        pending = deque(self._lineage_parent_ids(track))
        while pending:
            track_id = pending.popleft()
            if track_id in visited:
                continue
            visited.add(track_id)
            parent = find_track(self.project, track_id)
            if parent is None:
                continue
            if parent.type in {TrackType.GENERATED, TrackType.SOURCE}:
                yield parent
            pending.extend(self._lineage_parent_ids(parent))

    @staticmethod
    def _lineage_parent_ids(track: Track) -> list[str]:
        parent_ids = list(track.input_track_ids)
        source_track_id = track.provenance.get("source_track_id", "")
        if source_track_id and source_track_id not in parent_ids:
            parent_ids.append(source_track_id)
        return parent_ids

    def _valid_audio_artifact_path(self, track: Track) -> Path:
        entries = {entry.id: entry for entry in self.project.cache_entries}
        for cache_ref in track.cache_refs:
            entry = entries.get(cache_ref)
            if (
                entry is None
                or entry.artifact_kind not in {"audio", "stem"}
                or entry.validation_status != "valid"
            ):
                continue
            path = self.cache_store.artifact_path(entry)
            if path.is_file():
                return path
        raise ValueError(f"parent track has no valid audio artifact: {track.name}")
