# Autolight Rust CXX-Qt Port Design

Date: 2026-06-03

## Status

This is the active forward architecture for Autolight. Future app work should target the Rust/CXX-Qt version. The existing Python/PySide6 implementation remains the reference behavior and compatibility baseline until the Rust app reaches parity.

## Context

Autolight currently ships as a Python 3.14 application using PySide6, Qt Quick/QML, QtMultimedia, unittest, and JSON-backed `.autolight` project files. The implemented behavior includes graph-backed tracks, local jobs, cache artifacts, generated and editable marker tracks, direct timeline editing, undo/redo, timeline tree projection, music analysis artifacts, waveform LOD, playback controls, and a split QML component tree under `UI/components/`.

The UI investment is already Qt Quick/QML-shaped. Replacing both the application runtime and the UI toolkit at the same time would increase risk. The port should therefore keep Qt Quick/QML and replace Python/PySide with Rust exposed to QML through CXX-Qt.

## Goals

- Port the app runtime from Python/PySide6 to Rust while keeping Qt Quick/QML as the presentation layer.
- Use CXX-Qt as the Rust-to-Qt bridge for QObjects, properties, signals, invokables, and QML-facing models.
- Preserve `.autolight` project compatibility unless a later migration plan explicitly version-bumps the schema.
- Preserve the current QML component structure as much as practical, updating bindings only where the Rust bridge requires a different shape.
- Keep the Python app as a reference implementation until the Rust app passes parity gates.
- Move all continued product feature work to Rust/CXX-Qt after the port plan is active.
- Make the Rust codebase testable without launching QML for domain logic, cache behavior, transform logic, and timeline projection.
- Keep QtMultimedia/QML playback as the initial media playback surface unless the Rust spike proves that a Rust playback layer is necessary.

## Non-Goals

- Do not switch to Slint, egui, Iced, Tauri, or another UI stack during this port.
- Do not rewrite the visual design beyond binding and component adjustments needed for CXX-Qt.
- Do not introduce Python bridges for new forward behavior.
- Do not change the `.autolight` file extension, marker schema, track graph shape, or cache reference semantics during the initial port.
- Do not implement lighting-console export as part of the port.
- Do not require exact numeric equivalence for every audio-analysis heuristic before launching the Rust app, but preserve marker categories, artifact kinds, user-facing workflows, and deterministic test fixture expectations.

## Architecture

The Rust application should be a Cargo workspace with clear crates:

- `autolight-core`: project document types, track graph validation, marker mutation, edit history, cache metadata, dependency hashes, and transform registry types.
- `autolight-analysis`: waveform summaries, beat/energy/harmonic analysis boundaries, artifact payloads, and deterministic fixture algorithms.
- `autolight-jobs`: local job queue, cancellation, progress, rerun, stale propagation, and cache-write coordination.
- `autolight-qt`: CXX-Qt QObjects and QML-facing models for `AppController`, transform lists, timeline tracks, marker summaries, analysis slices, and waveform slices.
- `autolight-app`: binary entrypoint that creates the Qt application, registers CXX-Qt QML modules, loads `UI/Main.qml`, and implements smoke/screenshot commands.

Rust owns project truth, validation, jobs, cache, transform execution, timeline projection, and controller state. QML owns visual layout, pointer interaction, timeline drawing, dialogs, and QtMultimedia playback controls. QML must not run analysis or mutate project files directly.

## CXX-Qt Bridge Contract

The bridge should keep the current QML contract recognizable:

- `AppController` remains the single app-level object exposed to QML.
- Controller properties keep the same names where practical: project name/path, dirty state, selected track, marker summaries, undo/redo state, timeline duration, pixels-per-second, scroll seconds, visible seconds, and last error.
- Controller invokables keep the same names where practical: new/open/save/import, add transform, run/rerun/cancel, select track, create manual track, marker add/move/resize/delete, undo/redo, timeline zoom/scroll, playback seek, cache check, and tree expansion.
- QML-facing list models expose the same semantic roles: track ID, name, type, result state, marker spans, editability, waveform samples, tree depth, parent ID, expanded state, child state summary, tree errors, energy samples, and harmonic color samples.
- Rust domain code validates every committed mutation. QML can preview pointer state, but project mutation happens through Rust invokables.

If a CXX-Qt type cannot expose a role or method in the same shape, the Rust port may add a narrow QML adapter. Adapter changes must be covered by smoke or QML structural tests.

## Migration Strategy

The migration should proceed by behavior slices rather than by copying files mechanically.

1. Prove CXX-Qt can load the existing QML shell and expose a minimal controller plus track model.
2. Port `.autolight` project types and JSON round trips.
3. Port graph and marker mutation helpers, including editable track behavior and undo/redo.
4. Port cache metadata and artifact validation.
5. Port transform registry, local jobs, cancellation, progress, rerun, and stale propagation.
6. Port timeline projection roles, tree expansion state, visible waveform slices, and analysis strip slices.
7. Port QML controller slots and models until the existing QML components run against Rust.
8. Port waveform and music analysis algorithms enough to match current fixture expectations and user-facing artifact kinds.
9. Remove Python/PySide from the runtime path only after parity gates pass.

Each slice should include tests that compare Rust behavior to explicit expected values, not only to the Python implementation.

## Project And Cache Compatibility

Rust must read existing `.autolight` JSON files with schema version 1. It should preserve unknown optional `ui_state`, `metadata`, and provenance keys when saving. Cache references remain paths recorded in project documents plus artifact payloads on disk.

Transform output schemas remain stable:

- `artifact.waveform.v1`
- `artifact.audio.v1`
- `artifact.stem.v1`
- `artifact.beat-grid.v1`
- `artifact.energy.v1`
- `artifact.harmonic-color.v1`
- generated marker outputs using `markers.v1`

Cache artifact kinds remain stable and separate from output schemas:

- `waveform`
- `audio`
- `stem`
- `beat-grid`
- `energy`
- `harmonic-color`
- `markers`

Cache corruption handling remains recoverable: missing or invalid artifacts mark affected generated tracks stale and preserve visible markers plus editable descendants.

## Testing And Verification

Rust verification should include:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo test --workspace --locked`
- Qt/QML smoke launch of the Rust binary in offscreen mode.
- A Rust screenshot or structural UI check that proves the QML shell loads with a Rust controller.
- Fixture tests for `.autolight` round trips, graph validation, marker edits, undo/redo, cache validation, job state transitions, transform outputs, timeline tree projection, and visible artifact slices.

Python verification should remain available during the transition:

- `uv run python -m unittest discover -s tests -v`
- `QT_QPA_PLATFORM=offscreen uv run python main.py --smoke`

The Python verification commands are transition checks only. Passing Python tests does not authorize new product work in Python after this decision.

## Documentation Policy

Historical Python/PySide implementation plans remain useful records of completed behavior. They should not be rewritten as Rust tasks unless the Rust implementation is actively porting that slice. New forward work is routed through `docs/NOW.md` and must target Rust/CXX-Qt. Historical Python plans should be linked only as behavior references.

When a future batch touches app behavior, it must answer:

- Which Rust crate owns the behavior?
- Which CXX-Qt object or QML model exposes it?
- Which existing Python behavior or test defines parity?
- Which Rust verification command proves the behavior?

## Approved Decisions

- Lock in CXX-Qt as the Rust bridge.
- Keep Qt Quick/QML as the UI layer.
- Treat Slint as a rejected option for this port, not as a hidden fallback.
- Treat Python/PySide6 as the reference implementation, not the forward runtime.
- Move all continued app product work to Rust/CXX-Qt.
- Preserve project compatibility and current user workflows before adding new product scope.
