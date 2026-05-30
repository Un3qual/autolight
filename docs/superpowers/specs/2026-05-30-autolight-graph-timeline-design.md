# Autolight Graph Timeline Design

Date: 2026-05-30

## Context

Autolight is a Python desktop GUI app for analyzing music files and producing metadata-enriched timestamp markers for automated light shows. The long-term product goal is a non-linear-editor-like timeline where each track is either the source song, an analysis transform of another track, or an editable derivative. The first architecture milestone should build the transform graph and project model that make that timeline possible.

The current repository is a small PySide6/QML skeleton with `main.py`, `UI/Main.qml`, a starter notebook, and audio analysis dependencies already locked in `uv.lock`: `librosa`, `essentia`, `numpy`, `matplotlib`, `scikit-learn`, and `pyside6`.

## Goals

- Use PySide6/QML as the desktop application framework.
- Model the app as a graph-first audio analysis editor, with the timeline as a projection of graph state.
- Support generated analysis tracks, editable derived/manual tracks, local background jobs, cache-backed artifacts, and rich internal project persistence.
- Include source separation as a first-class MVP capability, even though it may be slow and asynchronous.
- Save enough structured project data to preserve reproducibility, provenance, marker metadata, and cache relationships.

## Non-Goals

- External lighting-console exports are not part of the first milestone.
- Multi-input graph transforms are not implemented in the first milestone.
- Polished NLE editing, advanced timeline gestures, and full visual design are deferred until the graph-backed shell is working.
- Remote workers and distributed processing are deferred. The first execution model is local background jobs.

## Architecture

The core application should be a Python domain engine with a Qt/QML presentation layer. Python owns project state, audio analysis, background execution, caching, and persistence. QML owns rendering, timeline interaction, and controls. QML should not run audio analysis directly.

The central abstraction is `TrackGraph`. A project contains imported audio assets and graph tracks. A track can be:

- `source`: a root track backed by an imported audio file.
- `generated`: a reproducible transform output derived from a parent track.
- `editable`: a user-owned marker track derived from generated output or created manually.

Each track stores `input_track_ids` as a list. The first implementation validates that generated tracks have one input, but the project schema stays compatible with future DAG transforms.

Generated tracks are read-only and reproducible. Rerunning a generated transform can replace its generated output. Editable tracks are not overwritten by reruns. They keep provenance back to source generated markers plus their own user edits.

## Components

`autolight.project` owns project document load/save, schema versioning, audio asset records, track graph records, marker records, run summaries, UI layout state, and cache metadata references.

`autolight.analysis` owns the transform registry and built-in transforms. Each transform declares its input requirements, parameter schema, output schema, estimated cost, transform version, and execution function.

`autolight.jobs` owns the local background worker queue. It handles queued, running, completed, failed, cancelled, stale, and blocked states. It also handles progress reporting, cancellation, dependency invalidation, and run history.

`autolight.cache` owns content-addressed artifact storage for expensive outputs such as decoded audio summaries, stems, feature arrays, marker sets, spectrogram or waveform preview data, and intermediate transform artifacts.

`autolight.timeline` owns Python view models that adapt project, graph, job, cache, and marker state into QML-friendly track rows, marker lanes, progress overlays, and inspector data.

`UI/` owns the QML timeline shell, track list, ruler, marker lanes, inspector panels, menus, and actions.

## Data Model

The internal project format should optimize for fidelity over interop. The recommended file extension is `.autolight`. It should be a structured project document with a schema version and references to cache artifacts. The first implementation can use JSON for clarity and testability, with a path to move to a bundled archive later if needed.

Core records:

- `Project`: schema version, project ID, audio assets, tracks, markers, job summaries, cache metadata, and UI state.
- `AudioAsset`: asset ID, source file path, duration, sample rate, channel count, content fingerprint, import status, and relink metadata.
- `Track`: track ID, track type, display name, input track IDs, transform ID, transform parameters, transform version, output schema, dependency hash, result state, cache references, and provenance.
- `Marker`: marker ID, track ID, timestamp, optional duration, label, category, confidence, tags, source transform, source marker IDs, and metadata payload.
- `JobRun`: run ID, track ID, transform ID, parameters hash, started/completed timestamps, state, progress summary, error summary, and produced cache references.
- `CacheEntry`: cache ID, dependency hash, artifact kind, file path, creation timestamp, transform version, size summary, and validation status.

## Transform Flow

Importing audio creates an `AudioAsset` and a root source track. The timeline can render that source track immediately with a loading waveform state, then update as waveform summaries are generated.

Adding a transform track records the parent input, transform ID, parameters, output schema, and dependency hash. If a matching cache entry exists, the track can become complete immediately. Otherwise, the jobs module queues a local background run.

Completed jobs write artifacts to the cache and write result references back into the project. The timeline model publishes the new state to QML. Failed jobs preserve the track and expose actionable errors.

Changing a parent track, transform version, or transform parameters invalidates dependent generated tracks. Stale tracks keep previous output visible when available, but they are clearly marked stale. The user can rerun or delete them. Editable tracks remain stable and keep provenance even if their generated source becomes stale.

## MVP Transform Set

The first design should treat these transform families as first-class:

- Timing: waveform summary, tempo, beats, downbeats, onsets, and coarse sections.
- Tonal and pitch: pitch estimates, chroma/key, confidence values, and melodic contours where available.
- Audio decomposition: harmonic/percussive separation.
- Source separation: vocals or stems as expensive asynchronous artifacts.
- Marker derivation: transforms that convert feature data into timestamped marker sets for timeline display.

Source separation should use the same transform/job/cache contract as lighter analyses. It can be implemented with a basic or mock-heavy path first, as long as the job and cache behavior matches the intended expensive-transform lifecycle.

## Timeline Behavior

The first QML UI should be a real timeline shell rather than a polished editor. It should show imported source tracks, generated transform tracks, editable marker tracks, markers at timestamps, stale/error/progress state, and an inspector for selected tracks or markers.

The timeline should support:

- Opening a project and importing one local audio file.
- Adding a transform track from a selected parent track.
- Showing generated markers and regions on lanes.
- Creating an editable derived marker track from generated output.
- Preserving manual edits across generated-track reruns.
- Saving and loading the project with graph, markers, and cache references intact.

Advanced editing gestures, complex snapping, export workflows, and final visual polish are later milestones.

## Error Handling

Analysis failures should be represented as track/job state, not application crashes. A failed transform remains in the graph, displays its error in the timeline or inspector, and offers rerun, parameter editing, or deletion.

Imports should validate that the file exists, is decodable, and can be fingerprinted. If a project opens and an audio file is missing, the asset is marked offline and the user can relink it.

Cache corruption or missing artifacts should be recoverable. If an artifact cannot be loaded, affected generated tracks are marked as needing rerun. Manual/editable marker data must survive if it is embedded in the project.

Cancellation should leave the project consistent. Partial output should not become the current result unless the transform explicitly supports resumable artifacts and marks them as valid.

## Testing

Unit tests should cover:

- Project schema round trips.
- Track graph validation.
- Single-parent dependency chains.
- Stale-state propagation.
- Marker provenance and editable-derived behavior.
- Cache key generation.
- Transform registry metadata.
- Job state transitions for queued, running, completed, failed, and cancelled states.

Transform tests should use tiny synthetic or fixture audio files so they remain deterministic and fast. Source separation can initially use a lightweight stand-in transform to validate async behavior and cache semantics.

UI tests can start narrow: verify that QML launches, timeline view models expose tracks and markers, and job state changes reach the view model.

## Milestone 1 Acceptance Criteria

- A user can import one local audio file.
- The app creates a graph-backed source track.
- The user can add generated transform tracks from that source or from another generated track.
- Local background jobs run transforms without blocking the UI.
- At least one expensive-transform path exercises async progress, cancellation, and cache behavior.
- Generated markers appear on a QML timeline shell.
- The user can create an editable derived marker track from generated markers.
- Saving and loading a `.autolight` project preserves assets, tracks, markers, provenance, cache references, and UI state needed for the shell.
- Missing or invalid cache artifacts are detected and recoverable through rerun state.

## Approved Decisions

- Long-term product direction is the NLE-like timeline.
- The first implementation should accomplish the transform graph foundation that the timeline depends on.
- Generated outputs stay reproducible and read-only.
- User edits happen in editable derived/manual tracks.
- The first graph implementation supports single-parent chains while storing inputs as lists for future DAG support.
- The project format optimizes for internal fidelity first.
- Source separation is in the MVP capability set.
- Execution is local background jobs.
- PySide6/QML is the committed UI stack.
