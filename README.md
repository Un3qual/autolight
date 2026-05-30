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

- Import one local audio file into a project.
- Create graph-backed source, generated, and editable tracks.
- Run deterministic built-in transforms through a local background job queue.
- Persist `.autolight` project files as JSON.
- Render project tracks and marker counts in a QML timeline shell.
