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

- 2026-06-04: Addressed the latest unresolved Codex bot review threads on PR #13 after commit `32d9bd3`.
- Changes made: stopped rebuilding the Rust QML `trackModel` for viewport-only visible-row updates; relinked missing source WAVs from the opened project directory when metadata/fingerprint match; persisted produced artifact payload files before recording cache refs; made project saves use a same-directory temp file and atomic replace; validated project graph invariants during load/save; normalized Rust demo and legacy fixture waveform/energy/harmonic payloads into drawable QML sample fields; preserved stale pending generated tracks before starting queued jobs; corrected the Rust README workflow so it no longer advertises unsupported analysis transforms or background execution.
- Next batch: none. Push this follow-up, refresh GitHub review threads, reply/resolve the addressed bot comments, and report any external check failures that remain outside GitHub-visible inline comments.
- Verification:
  - `cargo test -p autolight-core --locked project`: passed, 11 tests.
  - `cargo test -p autolight-jobs --locked jobs_preserve_stale_pending_track_before_runner_starts`: passed.
  - `cargo test -p autolight-jobs --locked jobs`: passed, 19 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked controller`: passed, 36 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked timeline_rows`: passed, 7 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_rust_adapter_uses_controller_models_and_actions`: passed.
  - `cargo fmt --all -- --check`: passed.
  - `git diff --check`: passed.
  - `rg -n "readonly property string rustAdapterSource|Qt\.createQmlObject\(" UI`: passed, no matches.
  - `rg -n "set_timeline_visible_track_range\(firstRow, rowCount\).*reloadModels" UI/RustAdapter.qml`: passed, no matches.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 20 `autolight-analysis` tests, 2 `autolight-app` tests, 39 `autolight-core` tests, 19 `autolight-jobs` tests, and 44 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed and printed `Rust smoke loaded UI/Main.qml with Autolight.Qt AppController`; Qt printed non-fatal audio-device and existing missing `Sans Serif` font alias warnings.
  - `QT_QPA_PLATFORM=offscreen uv run python main.py --smoke`: passed outside the sandbox because `uv` needs access to `/Users/admin/.cache/uv`. Qt multimedia channel warnings were non-fatal.

- 2026-06-04: Cleaned up the Rust QML adapter after review follow-through.
- Changes made: removed the embedded `rustAdapterSource` QML string from `UI/Main.qml`, moved the Rust/CXX-Qt adapter bridge into `UI/RustAdapter.qml`, kept the main UI on a file-backed synchronous adapter component, and extended the QML structure test to prevent regressions to string-generated adapter code.
- Next batch: none. PR review-bot refresh remains the next external gate after this push.
- Verification:
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked qml_rust_adapter_uses_controller_models_and_actions`: passed.
  - `cargo fmt --all`: applied rustfmt changes.
  - `cargo fmt --all -- --check`: passed.
  - `git diff --check`: passed.
  - `rg -n "readonly property string rustAdapterSource|Qt\.createQmlObject\(" UI`: passed, no matches.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 39 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 20 `autolight-analysis` tests, 2 `autolight-app` tests, 37 `autolight-core` tests, 17 `autolight-jobs` tests, and 39 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed and printed `Rust smoke loaded UI/Main.qml with Autolight.Qt AppController`; Qt printed non-fatal audio-device and existing missing `Sans Serif` font alias warnings.
  - `QT_QPA_PLATFORM=offscreen uv run python main.py --smoke`: first failed inside the sandbox because `uv` could not access `/Users/admin/.cache/uv`; rerun outside the sandbox passed. Qt multimedia channel warnings were non-fatal.

- 2026-06-04: Addressed a fresh review-bot follow-through pass on PR #13 after commit `97c50c8`.
- Changes made: fixed job terminal failure/cancel descendant staleness, rejected negative produced marker timestamps, blocked job submission when inputs are not complete, refreshed/marked dirty when `run_next` errors after mutating project state, encoded local playback paths with URL-safe path segments, bounded Rust `markers.fixed_interval` generation to the Python reference marker cap, disabled reruns for incomplete inputs, invalidated snapshot undo after structural non-history project mutations, finalized persisted active jobs on open, validated cache artifact files during open, restored source/dependent tracks when offline audio comes back online, tolerated JSON roundtrip noise in audio duration comparisons, cleaned up DeepSource-flagged empty `JobRegistry::new()` test initializers, and fixed outside-diff Rust clippy findings that were not exposed as GitHub inline comments.
- Next batch: none. Final post-push GitHub refresh after replies/resolutions showed no unresolved bot review threads.
- Verification:
  - `cargo fmt --all`: applied rustfmt changes.
  - `cargo fmt --all -- --check`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
  - `cargo test -p autolight-jobs --locked jobs`: passed, 17 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 39 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 20 `autolight-analysis` tests, 2 `autolight-app` tests, 37 `autolight-core` tests, 17 `autolight-jobs` tests, and 39 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed and printed `Rust smoke loaded UI/Main.qml with Autolight.Qt AppController`; Qt printed non-fatal audio-device and existing missing `Sans Serif` font alias warnings.
  - `QT_QPA_PLATFORM=offscreen uv run python main.py --smoke`: first failed inside the sandbox because `uv` could not access `/Users/admin/.cache/uv`; rerun outside the sandbox passed. Qt multimedia channel warnings were non-fatal.
  - `git diff --check`: passed.
  - GitHub review-thread refresh after replies/resolutions: passed, `unresolved_bot_total: 0`.

- 2026-06-04: Addressed review-bot follow-through on PR #13.
- Changes made: fixed Rust-port review findings for persisted ID seeding, negative marker timestamps, job-run parameter persistence, job-run failure finalization, full cache digest IDs, graph child-state summary precomputation, checked waveform legacy integer fields, zero-count waveform buckets, beat-grid tempo estimation, boundary-only energy peaks, visible-row snapping, WAV import validation/fingerprinting, timeline artifact validation, QML visible track range wiring, WAV-only import filtering, unsupported transform failure handling, loaded-project audio status refresh, cache artifact file validation, QtMultimedia playback transport, root-load checks, and Qt exit-status propagation.
- Next batch: none. All current review-bot findings have code/test coverage or will be replied to with the pushed fix commit.
- Verification:
  - `cargo fmt --all`: applied rustfmt changes.
  - `cargo test -p autolight-core --locked graph`: passed, 8 tests.
  - `cargo test -p autolight-core --locked markers`: passed, 11 tests.
  - `cargo test -p autolight-core --locked cache`: passed, 6 tests.
  - `cargo test -p autolight-core --locked project`: passed, 9 tests.
  - `cargo test -p autolight-analysis --locked waveform`: passed, 11 tests.
  - `cargo test -p autolight-analysis --locked music`: passed, 9 tests.
  - `cargo test -p autolight-jobs --locked jobs`: passed, 13 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-qt --locked`: passed, 32 tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test -p autolight-app --locked`: passed, 2 tests.
  - `cargo fmt --all -- --check`: passed.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake cargo test --workspace --locked`: passed, including 20 `autolight-analysis` tests, 2 `autolight-app` tests, 37 `autolight-core` tests, 13 `autolight-jobs` tests, and 32 `autolight-qt` tests.
  - `QMAKE=/opt/homebrew/opt/qt/bin/qmake QT_QPA_PLATFORM=offscreen cargo run -p autolight-app -- --smoke`: passed and printed `Rust smoke loaded UI/Main.qml with Autolight.Qt AppController`; Qt printed non-fatal audio-device and existing missing `Sans Serif` font alias warnings.
  - `QT_QPA_PLATFORM=offscreen uv run python main.py --smoke`: first failed inside the sandbox because `uv` could not access `/Users/admin/.cache/uv`; rerun outside the sandbox passed. Qt multimedia channel warnings were non-fatal.

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
