# Autolight

Autolight is a PySide6/QML desktop app for building graph-backed audio analysis timelines. The first milestone focuses on a `.autolight` project model, generated and editable tracks, local background analysis jobs, cache-aware transform outputs, and a timeline shell.

## Run

```bash
uv run python main.py
```

For headless launch verification:

```bash
QT_QPA_PLATFORM=offscreen uv run python main.py --smoke
```

## Test

```bash
uv run python -m unittest discover -s tests -v
```

## Current Scope

- Create, open, and save `.autolight` project files.
- Import one local audio file into a project.
- Create graph-backed source, generated, and editable tracks.
- Run deterministic built-in transforms through a local background job queue.
- Persist project tracks, markers, provenance, job summaries, and cache references as JSON.
- Render project tracks and marker counts in a QML timeline shell.

## Basic Workflow

1. Launch the app with `uv run python main.py`.
2. Use `Import Audio` to add a local audio file as a source track.
3. With the source track selected, choose `Add Markers` to create a generated fixed-interval marker track.
4. Run the generated marker track by selecting it and choosing `Run`.
5. After completion, choose `Derive Editable` to create editable cue markers from the generated track.
6. Use `Save` or `Save As` to write a `.autolight` project file.
7. Use `Open` to reload a saved project.

## Cache Recovery

Autolight records generated artifact metadata in the `.autolight` project file and stores artifact bytes under the app runtime cache. If a cached artifact is missing or corrupted, `Check Cache` marks affected generated tracks as `stale` while preserving visible markers and editable derived tracks. Select a stale or failed generated track and choose `Rerun` to regenerate its output.
