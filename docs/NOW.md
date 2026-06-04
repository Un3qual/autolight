# Autolight NOW

Updated: 2026-06-04

## Active Batch: None

**Status:** complete

**Goal:** The Rust/CXX-Qt runtime cutover is complete for the current roadmap. No unblocked Rust-port batch remains in `docs/ROADMAP.md`.

## Current State

The Rust/CXX-Qt app is now the primary runtime path. The Python/PySide app remains checked in as the reference implementation and parity baseline.

Default run path:

```bash
QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo run -p autolight-app
```

Reference Python run path:

```bash
uv run python main.py
```

## Completion Update

- 2026-06-04: Completed `Rust Runtime Cutover`.
- Changes made: updated README to present the Rust/CXX-Qt binary as the primary app and Python/PySide as the reference app; kept the Rust and Python smoke/test commands explicit; folded in the runtime-cutover blocker for timeline viewport and snap parity by moving timeline zoom/scroll/visible-seconds state into the Rust controller, persisting zoom/scroll in `.autolight` UI state, routing `snap_timeline_time` through Rust, applying snap to single-marker moves, and guarding QML timeline list bindings against transient null values during Rust adapter reloads.
- Next batch: none. All `docs/ROADMAP.md` ready-queue items are complete.
- Verification:
  - `cargo fmt --all`: applied rustfmt changes.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 23 tests.
  - `cargo fmt --all -- --check`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 15 `autolight-analysis` tests, 35 `autolight-core` tests, 10 `autolight-jobs` tests, and 23 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed and printed `Rust smoke loaded UI/Main.qml with Autolight.Qt AppController`; the only warning was Qt's existing missing `Sans Serif` font alias warning.
  - `QT_QPA_PLATFORM=offscreen uv run python main.py --smoke`: first failed inside the sandbox because `uv` could not access `/Users/admin/.cache/uv`; rerun outside the sandbox passed. Qt multimedia channel warnings were non-fatal.
  - `git diff --check`: passed.

## Previous Batch

- 2026-06-04: Completed `Rust File And Playback Controller Actions`.
- Changes made: added Rust controller qproperties for project path, selected-track playability, playback source/position/duration/playing/error/volume state; wired Rust qinvokables for open/save/import, selected-track playback, loaded playback, pause/stop/seek/nudge, and volume; added minimal WAV probing and deterministic fingerprints for Rust audio import; connected the Rust QML adapter to file dialogs and playback controls while preserving the Python `appController` path; added focused Rust tests for audio import/playability, save/open roundtrip, playback state transitions, and QML adapter wiring.
- Verification:
  - `cargo fmt --all`: applied rustfmt changes.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 21 tests.
  - `cargo fmt --all -- --check`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 15 `autolight-analysis` tests, 35 `autolight-core` tests, 10 `autolight-jobs` tests, and 21 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed and printed `Rust smoke loaded UI/Main.qml with Autolight.Qt AppController`.
  - `QT_QPA_PLATFORM=offscreen uv run python main.py --smoke`: first failed inside the sandbox because `uv` could not access `/Users/admin/.cache/uv`; rerun outside the sandbox passed. Qt multimedia channel warnings were non-fatal.
  - `git diff --check`: passed.
