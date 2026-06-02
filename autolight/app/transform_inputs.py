from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from autolight.cache.store import CacheStore
from autolight.project.models import ProjectDocument, ResultState, Track, TrackType
from autolight.project.store import find_track


@dataclass(slots=True)
class TransformInputResolver:
    project: ProjectDocument
    cache_store: CacheStore

    def audio_path_for_track(self, track: Track) -> str:
        if track.type == TrackType.SOURCE:
            return self._source_audio_path(track)
        if track.result_state != ResultState.COMPLETE:
            raise ValueError(f"parent track is not complete: {track.name}")
        return str(self._valid_audio_artifact_path(track))

    def _source_audio_path(self, track: Track) -> str:
        asset_id = track.provenance.get("asset_id")
        for asset in self.project.audio_assets:
            if asset.id == asset_id:
                if asset.import_status != "online":
                    raise ValueError(f"source audio is not online: {track.name}")
                return asset.path
        for parent_id in track.input_track_ids:
            parent = find_track(self.project, parent_id)
            if parent is not None:
                return self.audio_path_for_track(parent)
        raise ValueError(f"source audio path not found for track: {track.name}")

    def _valid_audio_artifact_path(self, track: Track) -> Path:
        entries = {entry.id: entry for entry in self.project.cache_entries}
        for cache_ref in track.cache_refs:
            entry = entries.get(cache_ref)
            if entry is None or entry.artifact_kind != "audio" or entry.validation_status != "valid":
                continue
            path = self.cache_store.artifact_path(entry)
            if path.is_file():
                return path
        raise ValueError(f"parent track has no valid audio artifact: {track.name}")
