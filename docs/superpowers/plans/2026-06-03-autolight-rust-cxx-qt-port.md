# Autolight Rust CXX-Qt Port Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **Execution routing:** Do not use this long-form plan as the session dispatcher. Start from `docs/NOW.md`, which selects the active batch and verification commands. This file is background structure for the full migration.

**Goal:** Port the current Autolight app from Python/PySide6 to Rust while keeping Qt Quick/QML through CXX-Qt.

**Architecture:** Build a Rust Cargo workspace with domain crates for project state, analysis, jobs, and cache behavior, plus a CXX-Qt crate that exposes QObjects and QML-facing models. Keep the existing QML component tree as the presentation layer, and use the Python app only as the behavior and compatibility reference during the transition.

**Tech Stack:** Rust stable, Cargo workspace, CXX-Qt, Qt Quick/QML, QtMultimedia through QML, serde, serde_json, thiserror, anyhow for binaries, uuid, sha2, tempfile, unittest for transitional Python parity checks, and existing `.autolight` JSON fixtures.

---

## File Structure

- Create `Cargo.toml`: workspace manifest and shared package metadata.
- Create `crates/autolight-core/Cargo.toml`: core domain crate dependencies.
- Create `crates/autolight-core/src/lib.rs`: module exports and crate-level lint policy.
- Create `crates/autolight-core/src/project.rs`: project document structs, schema constants, JSON load/save helpers.
- Create `crates/autolight-core/src/graph.rs`: track graph validation, source ancestor resolution, tree projection, stale propagation.
- Create `crates/autolight-core/src/markers.rs`: marker validation, editable track helpers, move/resize/delete operations.
- Create `crates/autolight-core/src/history.rs`: undo/redo command stack for marker and manual-track edits.
- Create `crates/autolight-core/src/cache.rs`: cache metadata, dependency hash, artifact validation state.
- Create `crates/autolight-core/src/transforms.rs`: transform specs, transform registry, transform result structs.
- Create `crates/autolight-analysis/Cargo.toml`: analysis crate dependencies.
- Create `crates/autolight-analysis/src/lib.rs`: analysis exports.
- Create `crates/autolight-analysis/src/waveform.rs`: waveform pyramid generation, legacy payload parsing, visible slicing.
- Create `crates/autolight-analysis/src/music.rs`: rhythm, energy, and harmonic analysis boundaries with deterministic fixture behavior.
- Create `crates/autolight-jobs/Cargo.toml`: job crate dependencies.
- Create `crates/autolight-jobs/src/lib.rs`: local queue exports.
- Create `crates/autolight-jobs/src/queue.rs`: local job state, cancellation, progress, result application.
- Create `crates/autolight-qt/Cargo.toml`: CXX-Qt crate dependencies and build metadata.
- Create `crates/autolight-qt/build.rs`: CXX-Qt bridge build setup.
- Create `crates/autolight-qt/src/lib.rs`: QML module registration and bridge exports.
- Create `crates/autolight-qt/src/app_controller.rs`: CXX-Qt `AppController` object.
- Create `crates/autolight-qt/src/timeline_model.rs`: QML-facing timeline track model.
- Create `crates/autolight-qt/src/transform_model.rs`: QML-facing transform list model.
- Create `crates/autolight-app/Cargo.toml`: binary crate dependencies.
- Create `crates/autolight-app/src/main.rs`: Qt app startup, QML loading, smoke mode, screenshot mode.
- Modify `UI/Main.qml` and `UI/components/*.qml`: update bindings only where the Rust CXX-Qt bridge changes object or model access.
- Create `fixtures/projects/`: stable `.autolight` files copied from current Python test scenarios.
- Create `tests/rust_qml_smoke.rs` or an equivalent integration test path supported by the final CXX-Qt build.
- Modify `README.md`: keep Python commands under legacy reference app and add Rust port commands once the workspace exists.

## Task 1: CXX-Qt Feasibility Spike

**Files:**
- Create: `Cargo.toml`
- Create: `crates/autolight-qt/Cargo.toml`
- Create: `crates/autolight-qt/build.rs`
- Create: `crates/autolight-qt/src/lib.rs`
- Create: `crates/autolight-qt/src/app_controller.rs`
- Create: `crates/autolight-app/Cargo.toml`
- Create: `crates/autolight-app/src/main.rs`
- Modify: `UI/Main.qml` only if a minimal adapter import is required

- [ ] **Step 1: Create the Rust workspace skeleton**

Use a Cargo workspace with `resolver = "2"`, one CXX-Qt crate, and one app binary. Configure the app binary to depend on `autolight-qt`.

- [ ] **Step 2: Add a minimal CXX-Qt controller**

Expose an `AppController` with `projectName`, `lastError`, and a `newProject()` invokable. The initial implementation should return deterministic values so QML smoke can prove the bridge works.

- [ ] **Step 3: Load the existing QML shell from Rust**

Create a Rust binary that starts `QGuiApplication`, registers the CXX-Qt module, loads `UI/Main.qml`, supports `--smoke`, and exits successfully when root objects load.

- [ ] **Step 4: Run the bridge verification**

Run:

```bash
cargo fmt --all -- --check
cargo test --workspace --locked
QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke
```

Expected: all commands pass, and the QML shell can read at least one Rust controller property.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates UI/Main.qml
git commit -m "Spike CXX-Qt app shell"
```

## Task 2: Project Schema And JSON Compatibility

**Files:**
- Create: `crates/autolight-core/Cargo.toml`
- Create: `crates/autolight-core/src/lib.rs`
- Create: `crates/autolight-core/src/project.rs`
- Create: `fixtures/projects/basic_graph.autolight`
- Create: `fixtures/projects/tree_analysis.autolight`
- Modify: `crates/autolight-qt/Cargo.toml`

- [ ] **Step 1: Add project document structs**

Implement Rust structs for `ProjectDocument`, `AudioAsset`, `Track`, `Marker`, `JobRun`, and `CacheEntry` using serde. Field names must match the current JSON keys.

- [ ] **Step 2: Add fixture round-trip tests**

Add tests that load both fixture project files, verify schema version 1, verify source/generated/editable track counts, save to a temp file, reload, and compare stable semantic fields.

- [ ] **Step 3: Preserve optional dictionaries**

Represent metadata, provenance, transform params, and UI state as serde JSON maps so unknown keys survive a read/write cycle.

- [ ] **Step 4: Run project compatibility tests**

Run:

```bash
cargo test -p autolight-core project --locked
```

Expected: fixture load and round-trip tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/autolight-core fixtures/projects
git commit -m "Port project schema to Rust"
```

## Task 3: Graph, Marker Editing, And Undo/Redo

**Files:**
- Create: `crates/autolight-core/src/graph.rs`
- Create: `crates/autolight-core/src/markers.rs`
- Create: `crates/autolight-core/src/history.rs`
- Modify: `crates/autolight-core/src/lib.rs`

- [ ] **Step 1: Port graph helpers**

Implement single-parent graph validation, source ancestor resolution, child lookup, tree projection, expanded-state pruning, and stale propagation through generated descendants.

- [ ] **Step 2: Port marker mutation helpers**

Implement editable-track-only marker add, update, move, resize, delete, multi-select move validation, and manual track creation using the resolved source-audio ancestor.

- [ ] **Step 3: Port undo/redo commands**

Implement command objects for manual track creation, marker add, marker delete, marker move, marker resize, inspector edits, and bulk metadata edits. Drag operations should record one command at release time.

- [ ] **Step 4: Add Rust behavior tests**

Cover generated-track immutability, manual track provenance, atomic multi-marker moves, invalid timestamps, resize validation, undo stack clearing on project replacement, and redo invalidation after new edits.

- [ ] **Step 5: Run graph and editing tests**

Run:

```bash
cargo test -p autolight-core graph --locked
cargo test -p autolight-core markers --locked
cargo test -p autolight-core history --locked
```

Expected: graph, marker, and history tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/autolight-core
git commit -m "Port graph and marker editing core"
```

## Task 4: Cache, Transform Registry, And Local Jobs

**Files:**
- Create: `crates/autolight-core/src/cache.rs`
- Create: `crates/autolight-core/src/transforms.rs`
- Create: `crates/autolight-jobs/Cargo.toml`
- Create: `crates/autolight-jobs/src/lib.rs`
- Create: `crates/autolight-jobs/src/queue.rs`
- Modify: `crates/autolight-core/src/lib.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Port cache metadata and dependency hashes**

Implement deterministic dependency hashes, cache entry validation states, missing-artifact detection, and artifact-kind lookup by cache reference.

- [ ] **Step 2: Port transform registry types**

Implement transform specs with ID, version, display name, input schema, output schema, estimated cost, and executable function boundary.

- [ ] **Step 3: Port local job queue semantics**

Implement queued, running, complete, failed, cancelled, stale, and blocked states. Add cancellation checks, progress updates, and result application into project state.

- [ ] **Step 4: Add job and cache tests**

Cover progress updates, cancellation before and during work, failed transforms preserving track state, rerun replacing output references, cache corruption marking tracks stale, and child stale propagation.

- [ ] **Step 5: Run job and cache tests**

Run:

```bash
cargo test -p autolight-core cache transforms --locked
cargo test -p autolight-jobs --locked
```

Expected: cache, transform, and job tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/autolight-core crates/autolight-jobs Cargo.toml
git commit -m "Port cache transforms and local jobs"
```

## Task 5: Waveform And Music Analysis Artifacts

**Files:**
- Create: `crates/autolight-analysis/Cargo.toml`
- Create: `crates/autolight-analysis/src/lib.rs`
- Create: `crates/autolight-analysis/src/waveform.rs`
- Create: `crates/autolight-analysis/src/music.rs`
- Modify: `Cargo.toml`
- Modify: `crates/autolight-core/src/transforms.rs`

- [ ] **Step 1: Port waveform pyramid behavior**

Implement waveform payload parsing, pyramid generation, level selection from pixels-per-second, visible-range slicing, and legacy single-level fallback.

- [ ] **Step 2: Port analysis boundaries**

Implement rhythm, energy, and harmonic analysis result structs and deterministic fixture algorithms that emit the same artifact kinds and marker categories as the Python version.

- [ ] **Step 3: Register built-in transforms**

Register fixed interval markers, drums stand-in, vocals stand-in, onsets, beats, waveform summary, beat grid, energy profile, and harmonic color.

- [ ] **Step 4: Add artifact tests**

Cover waveform LOD selection, visible slicing, legacy fallback, artifact version fields, bounded dense arrays, cancellation checks, and marker category expectations for beat, energy, and harmonic outputs.

- [ ] **Step 5: Run analysis tests**

Run:

```bash
cargo test -p autolight-analysis --locked
cargo test -p autolight-core transforms --locked
```

Expected: analysis and transform tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/autolight-analysis crates/autolight-core Cargo.toml
git commit -m "Port waveform and music analysis artifacts"
```

## Task 6: CXX-Qt Controller And QML Models

**Files:**
- Modify: `crates/autolight-qt/src/app_controller.rs`
- Create: `crates/autolight-qt/src/timeline_model.rs`
- Create: `crates/autolight-qt/src/transform_model.rs`
- Modify: `crates/autolight-qt/src/lib.rs`
- Modify: `crates/autolight-qt/Cargo.toml`

- [ ] **Step 1: Expose controller properties and invokables**

Port project, selection, dirty state, error state, timeline viewport, undo/redo, import/open/save, transform, run/rerun/cancel, marker editing, cache check, tree expansion, and playback coordination methods into Rust CXX-Qt.

- [ ] **Step 2: Expose timeline model roles**

Expose track ID, name, type, result state, marker count, marker spans, error, job state, waveform slices, editability, parent ID, depth, child state summary, tree error, energy samples, and harmonic color samples.

- [ ] **Step 3: Expose transform model roles**

Expose transform ID, version, name, estimated cost, output schema, and selected-parent compatibility.

- [ ] **Step 4: Add bridge tests**

Add tests or smoke assertions that instantiate the Rust controller, load a fixture project, expose model row counts, invoke marker edits, and observe changed properties.

- [ ] **Step 5: Run CXX-Qt bridge tests**

Run:

```bash
cargo test -p autolight-qt --locked
QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke
```

Expected: bridge tests and offscreen smoke pass.

- [ ] **Step 6: Commit**

```bash
git add crates/autolight-qt
git commit -m "Expose Rust controller and models to QML"
```

## Task 7: QML Binding Parity

**Files:**
- Modify: `UI/Main.qml`
- Modify: `UI/components/AnalysisStrip.qml`
- Modify: `UI/components/MarkerBlock.qml`
- Modify: `UI/components/MarkerInspector.qml`
- Modify: `UI/components/PlaybackBar.qml`
- Modify: `UI/components/ProjectToolbar.qml`
- Modify: `UI/components/StatusFooter.qml`
- Modify: `UI/components/TimelineLane.qml`
- Modify: `UI/components/TimelineRuler.qml`
- Modify: `UI/components/TimelineView.qml`
- Modify: `UI/components/TrackRow.qml`
- Modify: `UI/components/TransformBar.qml`
- Modify: `UI/components/WaveformStrip.qml`
- Modify: `UI/qmldir`

- [ ] **Step 1: Update imports and controller access**

Switch QML imports and context access from PySide-provided objects to the CXX-Qt registered module while preserving component responsibilities.

- [ ] **Step 2: Align model role access**

Update any role access where CXX-Qt exposes lists or structs differently than PySide. Keep user-facing labels, buttons, and timeline interactions unchanged.

- [ ] **Step 3: Verify timeline interactions**

Run the Rust app and exercise project load, tree expansion, track selection, marker selection, marker move/resize/delete, undo/redo, transform run/rerun/cancel, zoom, scroll, and playback seek.

- [ ] **Step 4: Run QML smoke and screenshot checks**

Run:

```bash
QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke
QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --screenshot /tmp/autolight-rust.png
```

Expected: smoke exits 0 and screenshot file is non-empty.

- [ ] **Step 5: Commit**

```bash
git add UI crates/autolight-app crates/autolight-qt
git commit -m "Wire QML shell to Rust controller"
```

## Task 8: Runtime Cutover And Documentation

**Files:**
- Modify: `README.md`
- Modify: `pyproject.toml` only if Python dependencies are being moved to reference-only status in project metadata.
- Modify: `docs/superpowers/specs/2026-06-03-autolight-rust-cxx-qt-port-design.md`
- Modify: `docs/superpowers/plans/2026-06-03-autolight-rust-cxx-qt-port.md`

- [ ] **Step 1: Add Rust run and test commands to README**

Document `cargo run -p autolight-app`, `QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`, and Rust test commands. Keep Python commands under a reference-app heading.

- [ ] **Step 2: Mark Python runtime as reference-only**

Update docs to state that Python/PySide remains for parity verification and bug fixes only until removed by a later cleanup plan.

- [ ] **Step 3: Run full verification**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke
uv run python -m unittest discover -s tests -v
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
git diff --check
```

Expected: all commands pass. Any Python failure must be understood as either a legitimate reference-app regression or an intentional cutover change documented in this plan.

- [ ] **Step 4: Commit**

```bash
git add README.md docs pyproject.toml
git commit -m "Document Rust CXX-Qt cutover"
```

## Acceptance Criteria

- The Rust app loads the existing Qt Quick/QML shell through CXX-Qt.
- Rust owns project state, graph validation, markers, edit history, cache metadata, transform registry, local jobs, timeline projection, and QML-facing controller state.
- Existing `.autolight` project files load and save without losing schema version 1 data.
- Current app workflows remain available: import/open/save, add/run/rerun/cancel transforms, editable marker authoring, undo/redo, snapping, tree expansion, analysis strips, waveform LOD, cache recovery, playback controls, and smoke launch.
- Python/PySide is no longer the target for new product work after Rust parity is reached.
- Rust formatting, linting, unit tests, and offscreen QML smoke pass.
