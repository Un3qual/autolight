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
- Restore saved timeline zoom, horizontal scroll, and selected track when reopening a project.
- Render project tracks and marker counts in a QML timeline shell.

## Basic Workflow

1. Launch the app with `uv run python main.py`.
2. Use `Import Audio` to add a local audio file as a source track.
3. Select the source track and use `Play`, `Pause`, `Stop`, or the scrubber to inspect the audio.
4. Use the timeline zoom and horizontal navigation controls to inspect markers at the needed time scale.
5. With the source track selected, choose `Add Markers` or `Add Transform` to create generated marker tracks.
6. Run generated tracks by selecting them and choosing `Run`.
7. After completion, choose `Derive Editable` to create editable cue markers from a generated track.
8. Use `Save` or `Save As` to write a `.autolight` project file.
9. Use `Open` to reload a saved project.

Timeline zoom, horizontal scroll, and the selected track are stored in the `.autolight` project when you save.

## Cache Recovery

Autolight records generated artifact metadata in the `.autolight` project file and stores artifact bytes under the app runtime cache. If a cached artifact is missing or corrupted, `Check Cache` marks affected generated tracks as `stale` while preserving visible markers and editable derived tracks. Select a stale or failed generated track and choose `Rerun` to regenerate its output.
