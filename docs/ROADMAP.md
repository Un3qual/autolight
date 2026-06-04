# Autolight Roadmap

This is the ordered execution queue. `docs/NOW.md` owns the current batch; this file is used only to promote the next batch when NOW is complete, blocked, or stale.

## Direction

Forward app work targets Rust/CXX-Qt. Qt Quick/QML remains the UI layer. The Python/PySide implementation is a reference app and parity source.

## Ready Queue

### 1. Rust CXX-Qt Smoke Spike

**Status:** complete

Prove that a Rust binary can start Qt, load `UI/Main.qml`, expose a minimal Rust `AppController`, and pass offscreen smoke.

### 2. Rust Project Schema Fixture

**Status:** complete

Port `.autolight` schema version 1 to `autolight-core`, add fixture projects, and prove JSON round trips preserve unknown metadata/provenance/UI keys.

### 3. Rust Graph And Marker Core

**Status:** complete

Port graph validation, source ancestor resolution, tree projection, editable marker mutation, manual track creation, and undo/redo.

### 4. Rust Timeline Model MVP

**Status:** complete

Expose enough CXX-Qt model roles for QML to render the current demo track list and marker spans from Rust-owned state.

### 5. Rust Cache Jobs And Transforms

**Status:** complete

Port cache metadata, transform registry, local job queue, progress, cancellation, rerun, and stale propagation.

### 6. Rust Waveform And Analysis Artifacts

**Status:** complete

Port waveform LOD and deterministic beat/energy/harmonic artifact behavior needed for current app parity.

### 7. Rust CXX-Qt Controller And QML Models

**Status:** complete

Expose Rust controller state, transform/timeline model surfaces, and the narrow action invokables needed for QML to use Rust-owned project data.

### 8. Rust QML Binding Parity

**Status:** complete

Bind existing QML workflows to the Rust controller and models while preserving the Python reference path until parity gates pass.

### 9. Rust Marker And Tree Controller Actions

**Status:** complete

Wire marker selection/editing, derive-editable, undo/redo, and tree expansion through the Rust controller.

### 10. Rust File And Playback Controller Actions

**Status:** complete

Wire file open/save/import and playback transport through the Rust controller.

### 11. Rust Runtime Cutover

**Status:** complete

Make the Rust binary the primary run path after the parity gates pass. Keep Python only as an archived/reference implementation until a later cleanup removes it.

## Parking Lot

These are intentionally not active:

- Lighting export.
- Slint or other UI toolkit migration.
- Full source separation.
- Multi-input graph transforms.
- Advanced NLE features such as lasso, copy/paste, split/merge, automation curves, and lighting-console fixture profiles.
