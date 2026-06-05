# Autolight

Autolight is a desktop app for building graph-backed audio analysis timelines.

## Runtime Direction

Autolight runs through the Rust/CXX-Qt binary. Qt Quick/QML remains the UI layer, while Rust owns project, timeline, transform, marker-editing, file, job, cache, and playback controller behavior.

## Working On The Repo

Start from `docs/NOW.md`. It contains the one active implementation batch, target paths, verification commands, and handoff notes.

Use `docs/ROADMAP.md` only when `docs/NOW.md` is complete, blocked, or stale. Use `docs/PROCESS.md` for the lightweight batch and handoff rules.

## App

Install or expose a Qt 6 development package that provides `qmake`. On this machine Homebrew Qt 6 is used.

Run the app:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo run -p autolight-app
```

For headless launch verification:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke
```

Run the test suite:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked
```

## Current Scope

- Create, open, and save `.autolight` project files.
- Import one local audio file into a project.
- Create graph-backed source, generated, and editable tracks.
- Create blank manual cue tracks for direct authoring.
- Run deterministic fixed-interval marker transforms through the Rust local job queue.
- Persist project tracks, markers, provenance, job summaries, and cache references as JSON.
- Restore saved timeline zoom, horizontal scroll, and selected track when reopening a project.
- Render project tracks and marker counts in a QML timeline shell.
- Display generated and editable tracks as a nested transform tree.
- Render saved waveform, energy, and harmonic/color artifact previews when they exist in project data.
- Move, resize, select, and delete editable cue markers directly on the timeline.
- Undo and redo manual track and marker edits during the current app session.
- Snap editable marker movement to visible generated timing markers, with a modifier-key bypass for free placement.
- Render zoom-adaptive waveform detail from saved artifact payloads while keeping playback follow, scrubbing, and scrolling responsive.

## Basic Workflow

1. Launch the app with `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo run -p autolight-app`.
2. Use `Import Audio` to add a local audio file as a source track.
3. Select the source track and use `Play`, `Pause`, `Stop`, or the scrubber to inspect the audio.
4. Use the timeline zoom and horizontal navigation controls to inspect markers at the needed time scale.
5. With the source track selected, choose `Add Markers` or `Add Transform` to create generated marker tracks.
6. Run generated tracks by selecting them and choosing `Run`.
7. Expand or collapse parent tracks to inspect nested saved outputs.
8. After completion, choose `Derive Editable` to create editable cue markers from a generated track.
9. Choose `Manual Track` to create an empty editable cue track for direct authoring.
10. Click cues to select them, shift-click to multi-select, drag selected cues to move them, and drag cue edges to resize duration cues.
11. Use `Undo` and `Redo` to recover from marker and manual-track edits during the current session.
12. Use generated timing tracks as snap guides while editing; hold the snap-bypass modifier for free placement.
13. Use `Save` or `Save As` to write a `.autolight` project file.
14. Use `Open` to reload a saved project.

Timeline zoom, horizontal scroll, and the selected track are stored in the `.autolight` project when you save.

## Cache Recovery

Autolight records generated artifact metadata in the `.autolight` project file and stores produced artifact bytes under cache paths relative to the saved project directory. If a cached artifact is missing or corrupted, `Check Cache` marks affected generated tracks as `stale` while preserving visible markers and editable derived tracks. Select a stale or failed generated track and choose `Rerun` to regenerate its output.
