# Autolight Tree-Aware Music Analysis Design

Date: 2026-06-02

## Migration Status

This document records the current tree-aware analysis behavior and Python/PySide6 implementation assumptions. The active forward architecture is the Rust/CXX-Qt port in `docs/superpowers/specs/2026-06-03-autolight-rust-cxx-qt-port-design.md`. Future app work should keep the graph, tree, cache, and analysis behaviors described here, but implement new product work in the Rust/CXX-Qt version.

## Context

Milestone 2 made Autolight usable as an interactive timeline authoring tool: users can create manual cue tracks, edit markers directly, undo and redo edits, snap to generated timing markers, persist timeline viewport state, and inspect zoom-adaptive waveform detail.

The next milestone should deepen the timeline and analysis surfaces rather than move into export. Users need richer generated material to edit against, and generated tracks need to appear as a transform tree instead of a flat list. A common workflow should be visible and natural: import a song, create a drums extraction track, then run onset or energy analysis against only that drums track and see those child results nested under it.

## Goals

- Add a replaceable music analysis engine boundary implemented with librosa for this milestone.
- Add separate product transforms for beat grid, energy profile, and harmonic color.
- Store dense analysis output as bounded, versioned cache artifacts.
- Emit practical generated markers for timeline authoring: beats, downbeats, bars, energy peaks, builds, drops, and stable harmonic changes.
- Render dense energy and harmonic/color artifacts as timeline strips or overlays where available.
- Project existing graph-backed tracks as a collapsible timeline tree using `input_track_ids`.
- Route child transform inputs from the selected parent track, including parent audio artifacts, instead of always resolving analysis back to the original source audio.
- Preserve existing project, run, rerun, cache, stale, and editable-derived workflows.

## Non-Goals

- Lighting export remains deferred.
- Song-structure labels such as intro, verse, chorus, bridge, and drop remain deferred.
- Real source separation remains deferred. A stand-in audio artifact transform may be used only to prove tree routing and child analysis.
- Full stem-aware analysis remains deferred beyond the routing and UI foundation.
- The timeline model does not need to move immediately to `QAbstractItemModel`; a flattened expanded tree projection is acceptable for this milestone.
- Multi-input graph transforms remain deferred.

## Architecture

The project model already stores the important graph primitive: each generated or editable track can list parent tracks through `input_track_ids`. This milestone should keep that schema and add two layers on top of it:

- A music analysis engine boundary under `autolight/analysis/`.
- A tree-aware timeline projection over the existing track graph.

The music analysis engine should expose focused operations that return plain Python result objects:

- `analyze_rhythm(audio_path, settings)` for beat, tempo, beat-strength, and downbeat/bar-grid candidates.
- `analyze_energy(audio_path, settings)` for frame energy, onset density, novelty, and intensity events.
- `analyze_harmony(audio_path, settings)` for chroma frames, reduced color values, global key metadata, and stable harmonic-change candidates.

The first implementation uses librosa directly. The public boundary should not require callers to know librosa details, so a later engine can replace or augment it without changing transform callers.

Built-in transforms remain separate product surfaces:

- `music.beat_grid`
- `music.energy_profile`
- `music.harmonic_color`

Each transform should behave like existing generated tracks: it can be added under a parent track, run or rerun, cache its outputs, mark dependents stale, and expose markers through the existing marker pipeline.

## Timeline Tree

The timeline should present the graph as a tree rooted at source tracks. Generated and editable child tracks appear nested below their parent. For example:

```text
Song.wav
  Drums Stem
    Drum Onsets
    Drum Energy
  Beat Grid
  Harmonic Color
    Harmonic Change Cues
```

The first implementation can keep `TimelineTrackModel` as a `QAbstractListModel` and expose only visible rows from a flattened expanded tree. It should add roles such as:

- `parentTrackId`
- `depth`
- `hasChildren`
- `expanded`
- `childCount`
- `visibleChildStateSummary`

QML rows should indent labels by depth and add an expand/collapse control for rows with children. Collapsed children should stay active internally. If a hidden child is running, stale, failed, or blocked, the parent row should show enough aggregate status to make that visible.

The projection should be deterministic:

- Preserve existing project track order among siblings.
- Treat missing or invalid parents as root-level rows with an error indicator.
- Detect cycles defensively and render affected tracks as root-level problem rows rather than hiding them.
- Persist expanded/collapsed state in `ProjectDocument.ui_state` or controller session state according to the same saved-viewport rules already used by the timeline.

## Transform Input Routing

Current audio transforms resolve `audio.v1` inputs back to the source audio asset. That remains valid when running analysis directly under a source track, but child transforms must be able to analyze a parent artifact.

This milestone should add an input resolver that inspects the selected parent track and the transform input schema. It should support at least:

- Source audio parent to `audio.v1`: use the imported source file path.
- Generated audio-artifact parent to `audio.v1`: use the parent track's complete, valid cached audio artifact path.
- Generated marker parent to marker-oriented transforms: preserve the existing marker-track behavior where applicable.

The resolver should fail before job submission if the parent track is incompatible, incomplete, stale, failed, running, or missing a valid artifact. Failure messages should name the parent track and expected input type.

The stand-in stem transform can be used to prove this behavior if needed. The important milestone result is not high-quality source separation; it is that child transforms route to the parent artifact and appear nested in the UI.

## Beat Grid Output

`music.beat_grid` should produce both markers and an artifact.

Markers should include:

- Beat markers.
- Downbeat markers when confidence is sufficient.
- Bar-start markers derived from downbeats or a stable meter assumption.

Marker metadata should include useful fields such as beat index, bar index, estimated BPM, beat strength, meter, analysis source, and confidence. Categories should distinguish beat, downbeat, and bar timing markers so snapping and visual styling can treat them differently.

The dense artifact should include version, duration, frame or event times, tempo estimates, beat times, beat strengths, downbeat candidates, meter assumptions, and settings used for the run.

## Energy Output

`music.energy_profile` should write a dense artifact and emit cue-useful events.

The artifact should include frame times, normalized RMS or loudness-style energy, onset density, novelty, smoothed intensity, and settings. Payload size should be bounded by frame hop, decimation, and maximum frame count.

Markers should be conservative and practical:

- Energy peaks.
- Build starts.
- Drop or impact moments.
- Quiet or low-intensity moments when useful for cueing.

Each marker should include confidence and metadata describing which signal triggered it. The transform should prefer fewer stable markers over noisy dense marker spam.

## Harmonic Color Output

`music.harmonic_color` should write a dense chroma/color artifact and emit only stable harmonic-change markers.

The artifact should include frame times, chroma vectors, reduced color or tonal-centroid values, optional global key estimate, confidence or stability metrics, and settings. The timeline value is primarily the dense strip or overlay, not only marker events.

Harmonic-change markers should be emitted only when the change passes stability and duration thresholds. Marker metadata should include previous and next color/key summaries when available.

## Timeline Rendering

The timeline should continue rendering regular marker spans for generated and editable marker tracks. It should also support artifact-backed strips for dense analysis tracks:

- Energy tracks can render an intensity strip or curve-like bar layer.
- Harmonic tracks can render a color strip derived from chroma or tonal-color values.
- Beat grid tracks can render markers and optional beat-strength styling.

The first rendering pass should be structural and testable before becoming visually elaborate. The controller/model may expose bounded visible slices of artifact data in the same spirit as waveform LOD roles. Rendering should avoid loading or rebinding full dense payloads on every playback tick.

## Data And Cache Compatibility

Artifacts should be JSON for this milestone. Each payload should include:

- `version`
- `kind`
- `duration`
- `source_track_id`
- `source_transform_id`
- `settings`
- bounded arrays for frame or event data

Existing `.autolight` files should continue to load without migration. Tree projection is derived from existing track graph fields. New UI state keys should be optional and ignored by older files.

Cache invalidation should keep the existing behavior: rerunning or editing a parent marks child generated tracks stale. Rerunning a child should not require rerunning siblings.

## Error Handling

Analysis transforms should validate numeric settings before loading audio. They should reject non-finite hop sizes, thresholds, and maximum counts. They should check cancellation before and after expensive audio loading or feature extraction.

The input resolver should fail early for:

- Missing parent tracks.
- Parent tracks that are pending, running, failed, cancelled, stale, or blocked.
- Missing, invalid, or incompatible parent artifacts.
- Source-audio transforms with no resolvable audio path.
- Transform schemas that the resolver does not understand.

The timeline tree should not hide invalid graph state. Missing parents, cycles, and unsupported structures should render as visible problem rows with clear state or error text.

## Testing

Tests should cover:

- Music analysis engine methods on small deterministic audio fixtures.
- Built-in transform registration for `music.beat_grid`, `music.energy_profile`, and `music.harmonic_color`.
- Marker metadata and categories for beat, energy, and harmonic outputs.
- Artifact payload versioning, bounds, and cancellation behavior.
- Input routing from source audio parents and generated audio-artifact parents.
- Rejection of incompatible or stale parent inputs.
- Stale propagation through nested generated tracks.
- Timeline tree flattening, depth roles, sibling order, expand/collapse state, and cycle or missing-parent handling.
- QML wiring for indentation, expand/collapse controls, and aggregate child state.
- Artifact strip model roles for visible energy and harmonic slices.
- Existing project workflow, transform, cache, playback, editable marker, and smoke tests.

## Milestone 3 Acceptance Criteria

- A user can add beat grid, energy profile, and harmonic color tracks under a source audio track.
- A user can run those tracks and see useful generated markers plus cached dense artifacts.
- Harmonic and energy analysis can be represented as timeline strips or overlays, not only as marker counts.
- The timeline displays parent and child tracks as an expandable tree.
- A generated audio-artifact track can be used as the parent for another audio analysis transform, and the child transform analyzes that parent artifact rather than the original mix.
- Parent reruns mark nested child analysis tracks stale.
- Existing `.autolight` projects load without migration.
- Existing tests and headless smoke checks continue to pass.

## Approved Decisions

- Export is deferred.
- The next product priority is audio analysis depth, then timeline authoring depth, then workflow polish.
- Current analysis scope is beat/tempo, energy/intensity, and harmonic/color.
- Song structure and stem-aware analysis are later milestones.
- Use a replaceable analysis engine boundary, implemented with librosa for now.
- Use separate product transforms backed by a shared analysis engine.
- Dense artifacts plus generated marker tracks are both required.
- The timeline should become tree-aware so child transforms are visibly nested under their input tracks.
- Child transforms must route to compatible parent artifacts when applicable.
- Future analysis, tree, and timeline extensions target the Rust/CXX-Qt app. The existing Python/PySide6 implementation remains the behavior and project-compatibility baseline during the port.
